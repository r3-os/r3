//! Raspberry Pi Pico testing support
//!
//! This test runner target module communicates with the target through one USB
//! connection. The target side is RP2040 bootrom's PICOBOOT interface if the
//! target is in BOOTSEL mode or the test driver serial interface if the test
//! driver is currently running. It uses the PICOBOOT interface to transfer the
//! test driver to the target's on-chip RAM. After the test driver completes
//! execution, the test runner requests the test driver to reset the target into
//! BOOTSEL mode, preparing for the next test run.
//!
//! # Prerequisites
//!
//! One Raspberry Pi Pico board or any compatible board. The USB port must be
//! connected to the host computer. This test runner only uses the USB port to
//! simplify the usage.
//!
//! The Pico must first be placed into BOOTSEL mode so that the test runner can
//! load a program.
use anyhow::{anyhow, Context, Result};
use bytemuck::{Pod, Zeroable};
use std::{future::Future, mem::size_of};
use tokio::{
    io::{AsyncWriteExt, BufStream},
    task::spawn_blocking,
    time::sleep,
};
use tokio_serial::{SerialPortBuilderExt, SerialStream};

use super::{demux::Demux, jlink::read_elf, Arch, DebugProbe, LinkerScripts, Target};
use crate::utils::retry_on_fail_with_delay;

pub struct RaspberryPiPico;

impl Target for RaspberryPiPico {
    fn target_arch(&self) -> Arch {
        Arch::CORTEX_M0
    }

    fn cargo_features(&self) -> Vec<String> {
        vec!["board-rp_pico".to_owned()]
    }

    fn linker_scripts(&self) -> LinkerScripts {
        LinkerScripts::arm_m_rt(
            "
            MEMORY
            {
              /* Load the program to RAM */
              /* NOTE K = KiBi = 1024 bytes */
              FLASH : ORIGIN = 0x20000000, LENGTH = 200K
              RAM : ORIGIN = 0x20032000, LENGTH = 64K
            }

            /* This is where the call stack will be allocated. */
            /* The stack is of the full descending type. */
            /* NOTE Do NOT modify `_stack_start` unless you know what you are doing */
            _stack_start = ORIGIN(RAM) + LENGTH(RAM);
            "
            .to_owned(),
        )
    }
    fn connect(&self) -> std::pin::Pin<Box<dyn Future<Output = Result<Box<dyn DebugProbe>>>>> {
        Box::pin(retry_on_fail_with_delay(|| async {
            // Try connecting to the target. This is important if a test
            // driver is currently running because we have to reboot the
            // target before loading the new test driver.
            log::debug!("Attempting to connect to the target by two methods simultaneously.");
            let serial_async = spawn_blocking(open_serial);
            let picoboot_interface_async = spawn_blocking(open_picoboot);
            let (serial, picoboot_interface) = tokio::join!(serial_async, picoboot_interface_async);
            // ignore `JoinError`
            let (serial, picoboot_interface) = (serial.unwrap(), picoboot_interface.unwrap());

            let serial = match (serial, picoboot_interface) {
                (Ok(serial), Err(e)) => {
                    log::debug!(
                        "Connected to a test driver serial interface. Connecting to \
                        a PICOBOOT USB interface failed with the following error: {e}",
                    );
                    Some(BufStream::new(serial))
                }
                (Err(e), Ok(_picoboot_interface)) => {
                    log::debug!(
                        "Connected to a PICOBOOT USB interface. Connecting to \
                        a test driver serial interface failed with the following \
                        error: {e}",
                    );
                    None
                }
                (Err(e1), Err(e2)) => anyhow::bail!(
                    "Could not connect to a test driver serial interface \
                    nor a PICOBOOT USB interface. Please put your Pico into \
                    BOOTSEL mode before executing this command.\n\
                    \n\
                    Serial interface error: {e1}\n\n\
                    PICOBOOT interface error: {e2}",
                ),
                (Ok(_), Ok(_)) => anyhow::bail!(
                    "Connected to both of a test driver serial \
                    interface and a PICOBOOT USB interface. \
                    This is unexpected."
                ),
            };

            Ok(Box::new(RaspberryPiPicoUsbDebugProbe { serial }) as _)
        }))
    }
}

struct RaspberryPiPicoUsbDebugProbe {
    /// Contains a handle to the serial port if the test driver is currently
    /// running.
    ///
    /// Even if this field is set, the test driver's current state is
    /// indeterminate in general, so the target must be rebooted before doing
    /// anything meaningful.
    serial: Option<BufStream<SerialStream>>,
}

impl DebugProbe for RaspberryPiPicoUsbDebugProbe {
    fn program_and_get_output(
        &mut self,
        exe: &std::path::Path,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<super::DynAsyncRead<'_>>> + '_>> {
        let exe = exe.to_owned();
        Box::pin(async move {
            if let Some(serial) = self.serial.take() {
                // Discard any leftover data and unwrap `BufStream`
                let mut serial = serial.into_inner();

                // Reboot the target into BOOTSEL mode. This will sever the
                // serial connection.
                log::debug!(
                    "We know that a test driver is currently running on the target. \
                    We will request a reboot first."
                );
                serial.write_all(b"r").await.with_context(|| {
                    "Could not send a command to the test driver serial interface."
                })?;

                // Wait until the host operating system recognizes the USB device...
                sleep(DEFAULT_PAUSE).await;
            }

            program_and_run_by_picoboot(&exe).await.with_context(|| {
                format!(
                    "Failed to execute the ELF ƒile '{}' on the target.",
                    exe.display()
                )
            })?;

            // Wait until the host operating system recognizes the USB device...
            sleep(DEFAULT_PAUSE).await;

            let serial =
                retry_on_fail_with_delay(|| async { spawn_blocking(open_serial).await.unwrap() })
                    .await
                    .with_context(|| "Failed to connect to the test driver serial interface.")?;

            self.serial = Some(BufStream::new(serial));

            // Tell the test driver that we are now listening for the output
            log::debug!("Unblocking the test driver's output");
            let serial = self.serial.as_mut().unwrap();
            serial
                .write_all(b"g")
                .await
                .with_context(|| "Failed to write to the test driver serial interface.")?;
            serial
                .flush()
                .await
                .with_context(|| "Failed to write to the test driver serial interface.")?;

            // Now, pass the channel to the caller
            Ok(Box::pin(Demux::new(serial)) as _)
        })
    }
}

/// Locate and open the test driver serial interface. A test driver must be
/// running for this function to succeed.
fn open_serial() -> Result<SerialStream> {
    log::debug!("Looking for the test driver serial port");
    let ports = serialport::available_ports()?;
    let port = ports
        .iter()
        .find(|port_info| {
            log::trace!(" ...{port_info:?}");

            use serialport::{SerialPortInfo, SerialPortType, UsbPortInfo};
            matches!(
                port_info,
                SerialPortInfo {
                    port_type: SerialPortType::UsbPort(UsbPortInfo {
                        vid: 0x16c0,
                        pid: 0x27dd,
                        ..
                    }),
                    ..
                }
            ) ||
            // FIXME: Apple M1 work-around
            //        (`available_ports` returns incorrect `SerialPortType`)
            port_info.port_name.starts_with("/dev/tty.usbmodem")
        })
        .ok_or_else(|| anyhow!("Could not locate the test driver serial port."))?;
    log::debug!("Test driver serial port = {port:?}");

    // Open the serial port
    tokio_serial::new(&port.port_name, 115200)
        .open_native_async()
        .with_context(|| {
            format!(
                "Could not open the test driver serial port at path '{}'.",
                port.port_name
            )
        })
}

/// Program and execute the specified ELF file by PICOBOOT protocol.
async fn program_and_run_by_picoboot(exe: &std::path::Path) -> Result<()> {
    let picoboot_interface_async = retry_on_fail_with_delay(|| async {
        spawn_blocking(open_picoboot).await.unwrap() // ignore `JoinError`
    });
    let (picoboot_interface, loadable_code) = tokio::join!(picoboot_interface_async, read_elf(exe));
    let PicobootInterface {
        mut device_handle,
        out_endpoint_i,
        in_endpoint_i,
    } = picoboot_interface.with_context(|| "Failed to locate the PICOBOOT interface.")?;
    let loadable_code = loadable_code.with_context(|| "Failed to analyze the ELF file.")?;

    log::debug!("Transfering the image");
    for (region_data, region_addr) in loadable_code.regions.iter() {
        log::debug!(
            " ... 0x{region_addr:08x}..=0x{:08x}",
            region_addr + region_data.len() as u64 - 1
        );

        let hdr = PicobootCmd::new_write(*region_addr as u32, region_data.len() as u32);
        let (result, device_handle_tmp) =
            write_bulk_all(device_handle, out_endpoint_i, bytemuck::bytes_of(&hdr)).await;
        device_handle = device_handle_tmp;
        let num_bytes_written = result.with_context(|| "Failed to issue a 'write' command.")?;
        if num_bytes_written != 32 {
            anyhow::bail!("Short write ({num_bytes_written} < 32)");
        }

        let (result, device_handle_tmp) =
            write_bulk_all(device_handle, out_endpoint_i, region_data).await;
        device_handle = device_handle_tmp;
        let num_bytes_written =
            result.with_context(|| "Failed to transmit the 'write' command's payload.")?;
        if num_bytes_written != region_data.len() {
            anyhow::bail!("Short write ({num_bytes_written} < {})", region_data.len());
        }

        let (result, device_handle_tmp) = read_bulk_empty(device_handle, in_endpoint_i).await;
        device_handle = device_handle_tmp;
        result.with_context(|| "Failed to receive a success response.")?;
    }

    log::debug!(
        "Rebooting RP2040 to start execution at 0x{:08x}",
        loadable_code.entry
    );

    let hdr = PicobootCmd::new_reboot(loadable_code.entry as u32, 0x2004_2000, 100);
    let (result, _) = write_bulk_all(device_handle, out_endpoint_i, bytemuck::bytes_of(&hdr)).await;
    let num_bytes_written = result.with_context(|| "Failed to issue a 'reboot' command.")?;
    if num_bytes_written != 32 {
        anyhow::bail!("Short write ({num_bytes_written} < 32)");
    }

    Ok(())
}

const DEFAULE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

const DEFAULT_PAUSE: std::time::Duration = std::time::Duration::from_secs(1);

async fn write_bulk_all(
    device_handle: rusb::DeviceHandle<rusb::GlobalContext>,
    endpoint: u8,
    buf: &[u8],
) -> (rusb::Result<usize>, rusb::DeviceHandle<rusb::GlobalContext>) {
    let buf = buf.to_owned(); // sigh
    spawn_blocking(move || {
        let mut buf = &buf[..];
        let mut num_bytes_written = 0;

        log::trace!("write_bulk_all({endpoint})");

        while !buf.is_empty() {
            match device_handle.write_bulk(endpoint, buf, DEFAULE_TIMEOUT) {
                Ok(0) => break,
                Ok(num_bytes) => {
                    num_bytes_written += num_bytes;
                    buf = &buf[num_bytes..];
                }
                Err(e) => return (Err(e), device_handle),
            }
        }

        (Ok(num_bytes_written), device_handle)
    })
    .await
    .unwrap()
}

async fn read_bulk_empty(
    device_handle: rusb::DeviceHandle<rusb::GlobalContext>,
    endpoint: u8,
) -> (rusb::Result<()>, rusb::DeviceHandle<rusb::GlobalContext>) {
    spawn_blocking(move || {
        log::trace!("read_bulk_empty({endpoint})");

        let result = match device_handle.read_bulk(endpoint, &mut [], DEFAULE_TIMEOUT) {
            Ok(0) => Ok(()),
            Ok(_) => unreachable!(),
            Err(e) => Err(e),
        };
        (result, device_handle)
    })
    .await
    .unwrap()
}

struct PicobootInterface {
    device_handle: rusb::DeviceHandle<rusb::GlobalContext>,
    out_endpoint_i: u8,
    in_endpoint_i: u8,
}

/// Locate and open the PICOBOOT interface. The device must be in BOOTSEL mode
/// for this function to succeed.
fn open_picoboot() -> Result<PicobootInterface> {
    // Locate the RP2040 bootrom device
    log::debug!("Looking for the RP2040 bootrom device");
    let devices = rusb::devices().with_context(|| "Failed to enumerate connected USB devices.")?;
    let device = devices
        .iter()
        .find(|device| {
            log::trace!(" ...{device:?}");
            let descriptor = match device.device_descriptor() {
                Ok(x) => x,
                Err(e) => {
                    log::warn!(
                        "Could not get the device descriptor of '{device:?}'; ignoring. Cause: {e}",
                    );
                    return false;
                }
            };

            descriptor.vendor_id() == 0x2e8a && descriptor.product_id() == 0x0003
        })
        .ok_or_else(|| anyhow!("Could not locate the RP2040 bootrom device."))?;

    log::debug!("Found the RP2040 bootrom device: {device:?}");

    // Locate the USB PICOBOOT interface
    log::debug!("Looking for the USB PICOBOOT interface");
    let config_desc = device.active_config_descriptor().with_context(|| {
        format!("Failed to get the active config descriptor of the device '{device:?}'.")
    })?;
    let interface = config_desc
        .interfaces()
        .filter_map(|interface| {
            // There should be exactly one interface configuration, so just
            // look at the first one
            interface
                .descriptors()
                .next()
                .filter(|interface_descriptor| {
                    log::trace!(" ...{interface_descriptor:?}");
                    (
                        interface_descriptor.class_code(),
                        interface_descriptor.sub_class_code(),
                        interface_descriptor.protocol_code(),
                    ) == (0xff, 0, 0)
                })
        })
        .next()
        // Fail if no eligible interface was found
        .ok_or_else(|| {
            anyhow!("Could not locate the RP2040 PICOBOOT interface from the device '{device:?}'.",)
        })?;
    let interface_i = interface.interface_number();
    log::debug!("PICOBOOT interface number = {interface_i}");

    // Locate the endpoints
    log::debug!("Looking for the USB PICOBOOT endpoints");
    let out_endpoint_i = interface
        .endpoint_descriptors()
        .find(|endpoint_descriptor| {
            log::trace!(" ...{endpoint_descriptor:?}");
            endpoint_descriptor.direction() == rusb::Direction::Out
        })
        .ok_or_else(|| {
            anyhow!(
                "Could not locate the RP2040 PICOBOOT BULK OUT endpoint from the device '{device:?}'.",
            )
        })?
        .address();
    log::debug!("PICOBOOT BULK OUT endpoint = {out_endpoint_i}");
    let in_endpoint_i = interface
        .endpoint_descriptors()
        .find(|endpoint_descriptor| {
            log::trace!(" ...{endpoint_descriptor:?}");
            endpoint_descriptor.direction() == rusb::Direction::In
        })
        .ok_or_else(|| {
            anyhow!(
                "Could not locate the RP2040 PICOBOOT BULK IN endpoint from the device '{device:?}'.",
            )
        })?
        .address();
    log::debug!("PICOBOOT BULK IN endpoint = {in_endpoint_i}");

    // Open the device
    let mut device_handle = device
        .open()
        .with_context(|| format!("Failed to open the device '{device:?}'."))?;

    // Claim the interface
    device_handle
        .claim_interface(interface_i)
        .with_context(|| {
            format!("Failed to claim the PICOBOOT interface (number {interface_i}).")
        })?;

    // Reset the PICOBOOT interface
    //
    // This request is handled by this code:
    // <https://github.com/raspberrypi/pico-bootrom/blob/00a4a19114/bootrom/usb_boot_device.c#L229>
    //
    // The RP2040 datasheet (release 1.2) says `bmRequestType` is `00100001b`,
    // but it's actually `01000001b`.
    // <https://github.com/raspberrypi/pico-feedback/issues/99>
    log::debug!("Sending INTERFACE_RESET");
    device_handle
        .write_control(0x41, 0x41, 0x0000, interface_i as u16, &[], DEFAULE_TIMEOUT)
        .with_context(|| format!("Failed to send INTERFACE_RESET to the device '{device:?}'."))?;

    Ok(PicobootInterface {
        device_handle,
        out_endpoint_i,
        in_endpoint_i,
    })
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PicobootCmd {
    _magic: u32,
    _token: u32,
    _cmd_id: u8,
    _cmd_size: u8,
    _reserved: u16,
    _transfer_length: u32,
    /// One of `PicobootCmdArgs*`
    _args: [u8; 16],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PicobootCmdArgsAddrSize {
    _addr: u32,
    _size: u32,
    _pad: [u8; 8],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PicobootCmdArgsReboot {
    _pc: u32,
    _sp: u32,
    _delay_ms: u32,
    _pad: [u8; 4],
}

const _: () = assert!(size_of::<PicobootCmd>() == 32);
const _: () = assert!(size_of::<PicobootCmdArgsAddrSize>() == 16);
const _: () = assert!(size_of::<PicobootCmdArgsReboot>() == 16);

impl PicobootCmd {
    /// Writes a contiguous memory range of memory (Flash or RAM) on the RP2040.
    fn new_write(addr: u32, size: u32) -> Self {
        Self {
            _magic: Self::magic(),
            _token: 0,
            _cmd_id: 0x05,
            _cmd_size: 0x08,
            _reserved: 0,
            _transfer_length: size,
            _args: bytemuck::cast(PicobootCmdArgsAddrSize {
                _addr: addr,
                _size: size,
                _pad: [0; _],
            }),
        }
    }

    /// Executes a function on the device.
    fn new_reboot(pc: u32, sp: u32, delay_ms: u32) -> Self {
        Self {
            _magic: Self::magic(),
            _token: 0,
            _cmd_id: 0x02,
            _cmd_size: 0x0c,
            _reserved: 0,
            _transfer_length: 0,
            _args: bytemuck::cast(PicobootCmdArgsReboot {
                _pc: pc,
                _sp: sp,
                _delay_ms: delay_ms,
                _pad: [0; _],
            }),
        }
    }

    fn magic() -> u32 {
        let value = 0x431fd10b;
        assert_eq!(
            value,
            u32::from_ne_bytes([0x0b, 0xd1, 0x1f, 0x43]),
            "a big endian host system is not supported, sorry!"
        );
        value
    }
}
