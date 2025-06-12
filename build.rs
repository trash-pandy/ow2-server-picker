use std::io::Write;

use winres;

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows" {
        let mut res = winres::WindowsResource::new();
        res.set_windres_path("/usr/bin/x86_64-w64-mingw32-windres")
            .set_manifest(
                r#"
                    <assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
                        <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
                            <security>
                                <requestedPrivileges>
                                    <requestedExecutionLevel level="requireAdministrator" uiAccess="false" />
                                </requestedPrivileges>
                            </security>
                        </trustInfo>
                    </assembly>
                "#,
            );
        match res.compile() {
            Err(error) => {
                write!(std::io::stderr(), "{}", error).unwrap();
                std::process::exit(1);
            }
            Ok(_) => {}
        }
    }
}
