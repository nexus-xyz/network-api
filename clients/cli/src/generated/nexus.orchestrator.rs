// This file is @generated by prost-build.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ProofRequest {
    #[prost(message, optional, tag = "1")]
    pub program: ::core::option::Option<CompiledProgram>,
    #[prost(message, optional, tag = "2")]
    pub input: ::core::option::Option<VmProgramInput>,
    /// Step of the trace to start the proof, inclusive.
    ///
    /// If missing, proving starts at the beginning of the trace.
    #[prost(int32, optional, tag = "3")]
    pub step_to_start: ::core::option::Option<i32>,
    /// Number of steps for this proof request.
    ///
    /// If zero, proving is skipped. If missing, all steps are proved.
    #[prost(int32, optional, tag = "4")]
    pub steps_to_prove: ::core::option::Option<i32>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ProofResponse {
    #[prost(message, optional, tag = "1")]
    pub proof: ::core::option::Option<Proof>,
}
/// A message that always represents a program runnable on the Nexus VM.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CompiledProgram {
    #[prost(oneof = "compiled_program::Program", tags = "1")]
    pub program: ::core::option::Option<compiled_program::Program>,
}
/// Nested message and enum types in `CompiledProgram`.
pub mod compiled_program {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Program {
        /// ELF binary containing a program to be proved, expressed in the RV32I ISA.
        #[prost(bytes, tag = "1")]
        Rv32iElfBytes(::prost::alloc::vec::Vec<u8>),
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct VmProgramInput {
    #[prost(oneof = "vm_program_input::Input", tags = "1")]
    pub input: ::core::option::Option<vm_program_input::Input>,
}
/// Nested message and enum types in `VMProgramInput`.
pub mod vm_program_input {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Input {
        /// Input expressed as raw bytes to be read as-is off of the input tape.
        #[prost(bytes, tag = "1")]
        RawBytes(::prost::alloc::vec::Vec<u8>),
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Proof {
    #[prost(oneof = "proof::Proof", tags = "1")]
    pub proof: ::core::option::Option<proof::Proof>,
}
/// Nested message and enum types in `Proof`.
pub mod proof {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Proof {
        #[prost(bytes, tag = "1")]
        NovaBytes(::prost::alloc::vec::Vec<u8>),
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ProgramSource {
    /// The source code to be compiled. There will be a variety of languages and
    /// ways to express everything a program needs for compilation (dependencies,
    /// multiple files, etc.) as our scope expands.
    #[prost(oneof = "program_source::Source", tags = "1")]
    pub source: ::core::option::Option<program_source::Source>,
}
/// Nested message and enum types in `ProgramSource`.
pub mod program_source {
    /// The source code to be compiled. There will be a variety of languages and
    /// ways to express everything a program needs for compilation (dependencies,
    /// multiple files, etc.) as our scope expands.
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Source {
        /// Option to use when the program in question can be expressed as a single
        /// rust file (i.e., a program written in the playground).
        #[prost(string, tag = "1")]
        RustSingleFile(::prost::alloc::string::String),
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CompileRequest {
    #[prost(message, optional, tag = "1")]
    pub source: ::core::option::Option<ProgramSource>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CompileResponse {
    #[prost(message, optional, tag = "1")]
    pub program: ::core::option::Option<CompiledProgram>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, Copy, PartialEq, ::prost::Message)]
pub struct Progress {
    /// Completion status expressed as a number between zero and one,
    /// inclusive.
    #[prost(float, tag = "1")]
    pub completed_fraction: f32,
    /// The total size of the execution trace in steps.
    #[prost(int32, tag = "2")]
    pub steps_in_trace: i32,
    /// The number of steps of the execution trace to be proven.
    #[prost(int32, tag = "3")]
    pub steps_to_prove: i32,
    /// The number of steps proven so far.
    #[prost(int32, tag = "4")]
    pub steps_proven: i32,
}
/// Streamed messages sent to the orchestrator to keep it updated with the
/// prover's status.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ProverRequest {
    #[prost(oneof = "prover_request::Contents", tags = "1, 2, 3, 4")]
    pub contents: ::core::option::Option<prover_request::Contents>,
}
/// Nested message and enum types in `ProverRequest`.
pub mod prover_request {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Contents {
        /// Details about this supply node for use by the orchestrator.
        #[prost(message, tag = "1")]
        Registration(super::ProverRequestRegistration),
        /// A completed proof.
        #[prost(message, tag = "2")]
        Proof(super::Proof),
        /// Periodic progress update for the current proof.
        #[prost(message, tag = "3")]
        Progress(super::Progress),
        /// Periodic liveness indicator when no proof is being computed.
        #[prost(message, tag = "4")]
        Heartbeat(super::Heartbeat),
    }
}
/// Metadata that helps the orchestrator schedule work to the requesting compute
/// supplier.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ProverRequestRegistration {
    /// What type of prover this is.
    #[prost(enumeration = "ProverType", tag = "1")]
    pub prover_type: i32,
    /// A unique identifier for this prover, generated by the prover.
    ///
    /// Distict provers must not share an identifier; do not use a constant value.
    #[prost(string, tag = "2")]
    pub prover_id: ::prost::alloc::string::String,
    /// The number of proof cycles that this prover expects to compute
    /// over the course of one second. Proof cycles are proof steps times k.
    #[prost(double, optional, tag = "3")]
    pub estimated_proof_cycles_hertz: ::core::option::Option<f64>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ProverResponse {
    /// Forward the literal request for now
    #[prost(message, optional, tag = "1")]
    pub to_prove: ::core::option::Option<ProofRequest>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, Copy, PartialEq, ::prost::Message)]
pub struct Heartbeat {}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum ProverType {
    /// Experimental new prover types should leave the prover type unspecified.
    Unspecified = 0,
    /// The default prover type, used for volunteered compute resources.
    Volunteer = 1,
    /// Provers running on public continuous integration.
    /// May restrict the types of programs that can be assigned.
    Ci = 2,
}
impl ProverType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            ProverType::Unspecified => "PROVER_TYPE_UNSPECIFIED",
            ProverType::Volunteer => "PROVER_TYPE_VOLUNTEER",
            ProverType::Ci => "PROVER_TYPE_CI",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "PROVER_TYPE_UNSPECIFIED" => Some(Self::Unspecified),
            "PROVER_TYPE_VOLUNTEER" => Some(Self::Volunteer),
            "PROVER_TYPE_CI" => Some(Self::Ci),
            _ => None,
        }
    }
}
