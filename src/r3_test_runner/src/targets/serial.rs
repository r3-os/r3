// Choose a serial port based on an environment variable named `SERIAL`.
pub fn choose_serial() -> Result<String, ChooseSerialError> {
    let var = match std::env::var("SERIAL") {
        Ok(x) if !x.is_empty() => Some(x),
        Ok(_) => {
            log::debug!("`SERIAL` is empty, performing automatic selection");
            None
        }
        Err(_) => {
            log::debug!("`SERIAL` is not a valid UTF-8, performing selection");
            None
        }
    };
    if let Some(p) = var {
        log::info!("Using the serial port {p:?} (manually selected)");
        Ok(p)
    } else {
        let ports = tokio_serial::available_ports()?;
        log::trace!("Available ports: {ports:?}");
        if ports.is_empty() {
            return Err(ChooseSerialError::NoPortsAvailable);
        } else if ports.len() > 1 {
            return Err(ChooseSerialError::MultiplePortsAvailable(
                ports.into_iter().map(|i| i.port_name).collect(),
            ));
        }

        let p = ports.into_iter().next().unwrap().port_name;
        log::info!("Using the serial port {p:?} (automatically selected)");
        Ok(p)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ChooseSerialError {
    #[error("No serial ports were found")]
    NoPortsAvailable,
    #[error(
        "Multiple serial ports were found. \
        Please specify one of the following by providing \
        environment variable `SERIAL`: {0:?}"
    )]
    MultiplePortsAvailable(Vec<String>),
    #[error("Could not enumerate serial ports.")]
    SystemError(
        #[from]
        #[source]
        tokio_serial::Error,
    ),
}
