use crate::traits::{DataDirection, NvmeTransport, TransportError};
use nvme_core::command::NvmeCommand;

pub struct MockTransport {
    pub commands: Vec<(NvmeCommand, Vec<u8>, DataDirection)>,
    pub responses: Vec<Result<Vec<u8>, TransportError>>,
    pub identify_response: Option<Vec<u8>>,
}

impl MockTransport {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            responses: Vec::new(),
            identify_response: None,
        }
    }

    pub fn with_identify(data: Vec<u8>) -> Self {
        Self {
            commands: Vec::new(),
            responses: Vec::new(),
            identify_response: Some(data),
        }
    }
}

impl NvmeTransport for MockTransport {
    fn send_admin_command(
        &mut self,
        cmd: &NvmeCommand,
        data: &[u8],
        dir: DataDirection,
    ) -> Result<Vec<u8>, TransportError> {
        self.commands.push((*cmd, data.to_vec(), dir));

        if !self.responses.is_empty() {
            self.responses.remove(0)
        } else {
            Ok(Vec::new())
        }
    }

    fn identify_controller(
        &mut self,
    ) -> Result<nvme_core::identify::IdentifyController, TransportError> {
        if let Some(data) = &self.identify_response {
            let id: nvme_core::identify::IdentifyController = unsafe {
                core::ptr::read(data.as_ptr() as *const _)
            };
            Ok(id)
        } else {
            Err(TransportError::NotFound)
        }
    }
}
