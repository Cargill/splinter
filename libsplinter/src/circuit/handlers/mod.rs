mod admin_message;
mod circuit_error;
mod circuit_message;
mod direct_message;
mod service_handlers;

pub use crate::circuit::handlers::admin_message::AdminDirectMessageHandler;
pub use crate::circuit::handlers::circuit_error::CircuitErrorHandler;
pub use crate::circuit::handlers::circuit_message::CircuitMessageHandler;
pub use crate::circuit::handlers::direct_message::CircuitDirectMessageHandler;
pub use crate::circuit::handlers::service_handlers::ServiceConnectForwardHandler;
pub use crate::circuit::handlers::service_handlers::ServiceConnectRequestHandler;
pub use crate::circuit::handlers::service_handlers::ServiceDisconnectForwardHandler;
pub use crate::circuit::handlers::service_handlers::ServiceDisconnectRequestHandler;
use crate::protos::circuit::{CircuitMessage, CircuitMessageType};
use crate::protos::network::{NetworkMessage, NetworkMessageType};

use protobuf::Message;

fn create_message(
    payload: Vec<u8>,
    circuit_message_type: CircuitMessageType,
) -> Result<Vec<u8>, protobuf::error::ProtobufError> {
    let mut circuit_msg = CircuitMessage::new();
    circuit_msg.set_message_type(circuit_message_type);
    circuit_msg.set_payload(payload);
    let circuit_bytes = circuit_msg.write_to_bytes()?;

    let mut network_msg = NetworkMessage::new();
    network_msg.set_message_type(NetworkMessageType::CIRCUIT);
    network_msg.set_payload(circuit_bytes);
    network_msg.write_to_bytes()
}
