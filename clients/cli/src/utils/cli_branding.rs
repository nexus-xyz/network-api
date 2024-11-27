use colored::Colorize;

pub const LOGO_NAME: &str = r#"
                                     ))[++                                     
                                 /)()))[+++++_                                 
                              |(())))))[++++++++<                              
                           )())))))))))[++++++++++++                           
                        )()))))))))))))[+++++++++++++~+                        
                     )())))))))))))))))[++++++++++++++++++                     
                 ()()))))))))))))))))))[++++++++++++++++++++++                 
              |))))))))))))))))))))))))[++++++++++++++++++++++~++              
            t))))))))))))))))))))))))))[++++++++++++++++++++++++++<            
            bbZv())))))))))))))))))))))[+++++++++++++++++++++++i:^^            
            bbbbbpc/)))))))))))))))))))[++++++++++++++++++~<!"^^^^^            
            bbbbbbbbd0|))))))))))))))))[++++++++++++++++~;^^^^^^^^^            
            bbbbbbbbbbbdQr)))))))))))))[+++++++++++++>;^^^^^^^^^^^^            
            bbbbbbbbbbbbbbbZr|)))))))))[++++++++~~>,^^^^^^^^^^^^^^^            
            bbbbbbbbbbbbbbbbbbpX())))))[++++++~l"^^^^^^^^^^^^^^^^^^            
            bbbbbbbbbbbbbbbbbbbbbpYt)))[+++<I"^^^^^^^^^^^^^^^^^^^^^            
            bbbbbbbbbbbbbbbbbbbbbbbbbQt[<;^^^^^^^^^^^^^^^^^^^^^^^^^            
            bbbbbbbbbbbbbbbbbbbbbbbbbdm1'`^^^^^^^^^^^^^^^^^^^^^^^^^            
            bbbbbbbbbbbbbbbbbbbbbbpwZZZ1....`^^^^^^^^^^^^^^^^^^^^^^            
            bbbbbbbbbbbbbbbbbbbpmZZZZZZ1.......`^^^^^^^^^^^^^^^^^^^            
            bbbbbbbbbbbbbbbdwmmZZZZZZZZ1..........'`^^^^^^^^^^^^^^^            
            bbbbbbbbbbbbdwZZZZZZZZZZZZZ1.............'`^^^^^^^^^^^^            
            bbbbbbbbbdmZZZZZZZZZZZZZZZZ1.................`^^^^^^^^^            
            bbbbbbpmmZZZZZZZZZZZZZZZZZZ1....................'^^^^^^            
            bbdpmZZZZZZZZZZZZZZZZZZZZZZ1.......................'`^^            
            mZZZZZZZZZZZZZZZZZZZZZZZZZZ1...........................            
              mmZZZZZZZZZZZZZZZZZZZZZZZ1.........................              
                 mmZZZZZZZZZZZZZZZZZZZZ1................... .                  
                     wmZZZZZZZZZZZZZZZZ1.................                      
                       mmmZZZZZZZZZZZZZ1..............                         
                           mmZZZZZZZZZZ1............                           
                              wmZZZZZZZ1.......                                
                                 wmZZZZ1.....                                  
                                     mZ1.                                      
                                                                               
                                                                               
                                                                               
00       00        0000000000     00       000     00        00       000000000
0000     00        00              000   000       00        00       00      0
00000    00        00                00 000        00        00       00       
00  000  00        000000000          000          00        00        0000000 
00    00 00        00                00 000        00        00              00
00     0000        00              000   000        00      00       00      00
00      000        0000000000     00       000       00000000         000000000
"#;

pub fn print_banner() {
    println!("{}", LOGO_NAME.bright_cyan());
    println!(
        "{} {}\n",
        "Welcome to the".bright_white(),
        "Nexus Network CLI".bright_cyan().bold()
    );
    println!(
        "{}",
        "The Nexus network is a massively-parallelized proof network for executing and proving the \x1b]8;;https://docs.nexus.org\x1b\\Nexus zkVM\x1b]8;;\x1b\\.\n\n"
            .bright_white()
    );
}

pub fn print_success(message: &str) {
    println!("✨ {}", message.green());
}

pub fn print_error(message: &str) {
    println!("❌ {}", message.red());
}
