//! Raspberry Pi Pico testing support
use anyhow::{anyhow, Context, Result};
use std::future::Future;
use tokio::task::spawn_blocking;

use super::{jlink::read_elf, Arch, DebugProbe, Target};

pub struct RaspberryPiPico;

impl Target for RaspberryPiPico {
    fn target_arch(&self) -> Arch {
        Arch::CORTEX_M0
    }

    fn cargo_features(&self) -> &[&str] {
        &["board-rp_pico"]
    }

    fn memory_layout_script(&self) -> String {
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
        .to_owned()
    }

    fn connect(&self) -> std::pin::Pin<Box<dyn Future<Output = Result<Box<dyn DebugProbe>>>>> {
        Box::pin(std::future::ready(Ok(
            Box::new(RaspberryPiPicoUsbDebugProbe) as _,
        )))
    }
}

struct RaspberryPiPicoUsbDebugProbe;

impl DebugProbe for RaspberryPiPicoUsbDebugProbe {
    fn program_and_get_output(
        &mut self,
        exe: &std::path::Path,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<super::DynAsyncRead<'_>>> + '_>> {
        let exe = exe.to_owned();
        Box::pin(async move {
            program_and_run_by_picoboot(&exe).await.with_context(|| {
                format!(
                    "Failed to execute the ELF ƒile '{}' on the target.",
                    exe.display()
                )
            })?;

            // TODO: Attach to the USB serial, give a 'go' signal, grab the output,
            //       and then issue a reboot request by sending `r`

            todo!()
        })
    }
}

/// Program and execute the specified ELF file by PICOBOOT protocol.
async fn program_and_run_by_picoboot(exe: &std::path::Path) -> Result<()> {
    let (picoboot_interface, loadable_code) =
        tokio::join!(spawn_blocking(open_picoboot), read_elf(exe));
    let picoboot_interface = picoboot_interface.unwrap(); // ignore `JoinError`
    let PicobootInterface {
        mut device_handle,
        out_endpoint_i,
        in_endpoint_i,
    } = picoboot_interface.with_context(|| {
        "Failed to locate the PICOBOOT interface. \
        Make sure to place your Pico into BOOTSEL mode before executing this command."
    })?;
    let loadable_code = loadable_code.with_context(|| "Failed to analyze the ELF file.")?;

    log::debug!("Transfering the image");
    for (region_data, region_addr) in loadable_code.regions.iter() {
        log::debug!(
            " ... 0x{:08x}..=0x{:08x}",
            region_addr,
            region_addr + region_data.len() as u64 - 1
        );

        let hdr = PicobootCmd::new_write(*region_addr as u32, region_data.len() as u32);
        let (result, device_handle_tmp) =
            write_bulk_all(device_handle, out_endpoint_i, hdr.as_bytes()).await;
        device_handle = device_handle_tmp;
        let num_bytes_written = result.with_context(|| "Failed to issue a 'write' command.")?;
        if num_bytes_written != 32 {
            anyhow::bail!("Short write ({} < 32)", num_bytes_written);
        }

        let (result, device_handle_tmp) =
            write_bulk_all(device_handle, out_endpoint_i, region_data).await;
        device_handle = device_handle_tmp;
        let num_bytes_written =
            result.with_context(|| "Failed to transmit the 'write' command's payload.")?;
        if num_bytes_written != region_data.len() {
            anyhow::bail!(
                "Short write ({} < {})",
                num_bytes_written,
                region_data.len()
            );
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
    let (result, _) = write_bulk_all(device_handle, out_endpoint_i, hdr.as_bytes()).await;
    let num_bytes_written = result.with_context(|| "Failed to issue a 'reboot' command.")?;
    if num_bytes_written != 32 {
        anyhow::bail!("Short write ({} < 32)", num_bytes_written);
    }

    Ok(())
}

async fn write_bulk_all(
    device_handle: rusb::DeviceHandle<rusb::GlobalContext>,
    endpoint: u8,
    buf: &[u8],
) -> (rusb::Result<usize>, rusb::DeviceHandle<rusb::GlobalContext>) {
    let buf = buf.to_owned(); // sigh
    spawn_blocking(move || {
        let timeout = std::time::Duration::from_secs(5);
        let mut buf = &buf[..];
        let mut num_bytes_written = 0;

        log::trace!("write_bulk_all({})", endpoint);

        while buf.len() > 0 {
            match device_handle.write_bulk(endpoint, buf, timeout) {
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
        let timeout = std::time::Duration::from_secs(5);

        log::trace!("read_bulk_empty({})", endpoint);

        let result = match device_handle.read_bulk(endpoint, &mut [], timeout) {
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

/// Locate the PICOBOOT interface.
fn open_picoboot() -> Result<PicobootInterface> {
    // Locate the RP2040 bootrom device
    log::debug!("Looking for the RP2040 bootrom device");
    let devices = rusb::devices().with_context(|| "Failed to enumerate connected USB devices.")?;
    let device = devices
        .iter()
        .find(|device| {
            log::trace!(" ...{:?}", device);
            let descriptor = match device.device_descriptor() {
                Ok(x) => x,
                Err(e) => {
                    log::warn!(
                        "Could not get the device descriptor of '{:?}'; ignoring. Cause: {}",
                        device,
                        e
                    );
                    return false;
                }
            };

            descriptor.vendor_id() == 0x2e8a && descriptor.product_id() == 0x0003
        })
        .ok_or_else(|| anyhow!("Could not locate the RP2040 bootrom device."))?;

    log::debug!("Found the RP2040 bootrom device: {:?}", device);

    // Locate the USB PICOBOOT interface
    log::debug!("Looking for the USB PICOBOOT interface");
    let config_desc = device.active_config_descriptor().with_context(|| {
        format!(
            "Failed to get the active config descriptor of the device '{:?}'.",
            device
        )
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
                    log::trace!(" ...{:?}", interface_descriptor);
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
            anyhow!(
                "Could not locate the RP2040 PICOBOOT interface from the device '{:?}'.",
                device
            )
        })?;
    let interface_i = interface.interface_number();
    log::debug!("PICOBOOT interface number = {}", interface_i);

    // Locate the endpoints
    log::debug!("Looking for the USB PICOBOOT endpoints");
    let out_endpoint_i = interface
        .endpoint_descriptors()
        .find(|endpoint_descriptor| {
            log::trace!(" ...{:?}", endpoint_descriptor);
            endpoint_descriptor.direction() == rusb::Direction::Out
        })
        .ok_or_else(|| {
            anyhow!(
                "Could not locate the RP2040 PICOBOOT BULK OUT endpoint from the device '{:?}'.",
                device
            )
        })?
        .address();
    log::debug!("PICOBOOT BULK OUT endpoint = {}", out_endpoint_i);
    let in_endpoint_i = interface
        .endpoint_descriptors()
        .find(|endpoint_descriptor| {
            log::trace!(" ...{:?}", endpoint_descriptor);
            endpoint_descriptor.direction() == rusb::Direction::In
        })
        .ok_or_else(|| {
            anyhow!(
                "Could not locate the RP2040 PICOBOOT BULK IN endpoint from the device '{:?}'.",
                device
            )
        })?
        .address();
    log::debug!("PICOBOOT BULK IN endpoint = {}", in_endpoint_i);

    // Open the device
    let mut device_handle = device
        .open()
        .with_context(|| format!("Failed to open the device '{:?}'.", device))?;

    // Reset the device
    device_handle
        .reset()
        .with_context(|| format!("Failed to reset the device '{:?}'.", device))?;

    // Claim the interface
    device_handle
        .claim_interface(interface_i)
        .with_context(|| {
            format!(
                "Failed to claim the PICOBOOT interface (number {}).",
                interface_i
            )
        })?;

    Ok(PicobootInterface {
        device_handle,
        out_endpoint_i,
        in_endpoint_i,
    })
}

#[repr(C)]
#[derive(Clone, Copy)]
struct PicobootCmd {
    _magic: u32,
    _token: u32,
    _cmd_id: u8,
    _cmd_size: u8,
    _reserved: u16,
    _transfer_length: u32,
    _args: PicobootCmdArgs,
}

#[repr(C)]
#[derive(Clone, Copy)]
union PicobootCmdArgs {
    _reboot: PicobootCmdArgsReboot,
    _addr_size: PicobootCmdArgsAddrSize,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct PicobootCmdArgsAddrSize {
    _addr: u32,
    _size: u32,
    _pad: [u8; 8],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct PicobootCmdArgsReboot {
    _pc: u32,
    _sp: u32,
    _delay_ms: u32,
    _pad: [u8; 4],
}

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
            _args: PicobootCmdArgs {
                _addr_size: PicobootCmdArgsAddrSize {
                    _addr: addr,
                    _size: size,
                    _pad: [0; 8],
                },
            },
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
            _args: PicobootCmdArgs {
                _reboot: PicobootCmdArgsReboot {
                    _pc: pc,
                    _sp: sp,
                    _delay_ms: delay_ms,
                    _pad: [0; 4],
                },
            },
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

    fn as_bytes(&self) -> &[u8; 32] {
        assert_eq!(std::mem::size_of::<Self>(), 32);
        unsafe { &*(self as *const Self as *const [u8; 32]) }
    }
}
