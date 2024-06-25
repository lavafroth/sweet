use anyhow::{bail, Result};
use std::path::Path;
use sweet::{ParserInput, SwhkdParser};

fn main() -> Result<()> {
    let Some(arg) = std::env::args().nth(1) else {
        bail!("please supply a path to a hotkeys config file");
    };
    let parser = SwhkdParser::from(ParserInput::Path(Path::new(&arg)))?;

    for binding in parser.bindings {
        println!("{}", binding);
    }
    for unbind in parser.unbinds {
        println!("unbind: {}", unbind);
    }
    for import in parser.imports {
        println!("import: {:?}", import);
    }
    for mode in parser.modes {
        println!("mode: {:?}", mode);
    }
    Ok(())
}
