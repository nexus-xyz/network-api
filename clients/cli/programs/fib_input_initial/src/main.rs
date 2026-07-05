use nexus_sdk::{
    ByGuestCompilation, Local, Prover, Verifiable, Viewable,
    compile::{Compile, Compiler, cargo::CargoPackager},
    stwo::seq::Stwo,
};

const PACKAGE: &str = "guest";

fn main() {
    println!("Compiling guest program...");
    let mut prover_compiler = Compiler::<CargoPackager>::new(PACKAGE);
    let prover: Stwo<Local> =
        Stwo::compile(&mut prover_compiler).expect("failed to compile guest program");

    let elf = prover.elf.clone(); // save elf for use with verification

    // Example: pass three inputs to the guest program
    let n = 9u32;
    let init_a = 1u32;
    let init_b = 1u32;
    let public_inputs = (n, init_a, init_b);

    println!("Proving execution of vm...");
    let (view, proof) = prover
        .prove_with_input::<(), (u32, u32, u32)>(&(), &public_inputs)
        .expect("failed to prove program");

    println!(
        ">>>>> Logging\n{}<<<<<",
        view.logs().expect("failed to retrieve debug logs").join("")
    );
    assert_eq!(
        view.exit_code().expect("failed to retrieve exit code"),
        nexus_sdk::KnownExitCodes::ExitSuccess as u32
    );

    // Normally the prover communicates the serialized proof to the verifier who deserializes it.
    //
    // The verifier must also possess the program binary and the public i/o. Usually, either
    // the verifier will rebuild the elf in a reproducible way (e.g., within a container) or
    // the prover will communicate it to the verifier who will then check that it is a valid
    // compilation of the claimed guest program. Here we simulate the latter.
    //
    // If we instead wanted to simulate the former, it might look something like:
    //
    // println!("Verifier recompiling guest program...");
    // let mut verifier_compiler = Compiler::<CargoPackager>::new(PACKAGE);
    // let path = verifier_compiler.build().expect("failed to (re)compile guest program");
    //
    // print!("Verifying execution...");
    // proof.verify_expected_from_program_path::<&str, (), ()>(
    //    &(),   // no public input
    //    nexus_sdk::KnownExitCodes::ExitSuccess as u32,
    //    &(),   // no public output
    //    &path, // path to expected program binary
    //    &[]    // no associated data,
    // ).expect("failed to verify proof");

    print!("Verifying execution...");
    #[rustfmt::skip]
    proof
        .verify_expected::<(u32, u32, u32), ()>(
            &public_inputs,  // three u32 inputs
            nexus_sdk::KnownExitCodes::ExitSuccess as u32,
            &(),  // no public output
            &elf, // expected elf (program binary)
            &[],  // no associated data,
        )
        .expect("failed to verify proof");

    println!("  Succeeded!");
}
