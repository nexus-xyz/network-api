// This file is @generated by prost-build.
/// Request a prover task.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetProofTaskRequest {
    /// This node's ID.
    #[prost(string, tag = "1")]
    pub node_id: ::prost::alloc::string::String,
    /// The type of this node.
    #[prost(enumeration = "NodeType", tag = "2")]
    pub node_type: i32,
}
/// A Prover task.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetProofTaskResponse {
    /// Program id. (Assuming client-side default programs)
    #[prost(string, tag = "1")]
    pub program_id: ::prost::alloc::string::String,
    /// Public inputs to the program.
    #[prost(bytes = "vec", tag = "2")]
    pub public_inputs: ::prost::alloc::vec::Vec<u8>,
    /// An id to submit along with the completed proof
    #[prost(uint64, tag = "3")]
    pub task_id: u64,
}
/// Submit the result of a prover task.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SubmitProofRequest {
    /// The prover node's ID.
    #[prost(string, tag = "1")]
    pub node_id: ::prost::alloc::string::String,
    /// The type of this node.
    #[prost(enumeration = "NodeType", tag = "2")]
    pub node_type: i32,
    /// Hash of the proof.
    #[prost(string, tag = "3")]
    pub proof_hash: ::prost::alloc::string::String,
    /// Telemetry data about the node
    #[prost(message, optional, tag = "4")]
    pub node_telemetry: ::core::option::Option<NodeTelemetry>,
    /// ZK proof of the proof activity
    #[prost(bytes = "vec", tag = "5")]
    pub proof: ::prost::alloc::vec::Vec<u8>,
    /// The id of the task being completed
    #[prost(uint64, tag = "6")]
    pub task_id: u64,
}
/// Performance stats of a node.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NodeTelemetry {
    /// Flops per second
    #[prost(int32, optional, tag = "1")]
    pub flops_per_sec: ::core::option::Option<i32>,
    /// Memory used in bytes for the proof activity
    #[prost(int32, optional, tag = "2")]
    pub memory_used: ::core::option::Option<i32>,
    /// Memory capacity in bytes of the node
    #[prost(int32, optional, tag = "3")]
    pub memory_capacity: ::core::option::Option<i32>,
    /// Geo location of the node
    #[prost(string, optional, tag = "4")]
    pub location: ::core::option::Option<::prost::alloc::string::String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum NodeType {
    /// The node is a web prover.
    WebProver = 0,
    /// The node is a CLI prover.
    CliProver = 1,
}
impl NodeType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Self::WebProver => "WEB_PROVER",
            Self::CliProver => "CLI_PROVER",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "WEB_PROVER" => Some(Self::WebProver),
            "CLI_PROVER" => Some(Self::CliProver),
            _ => None,
        }
    }
}
