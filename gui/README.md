# RFkit
For compiled executables, please go to [Releases](https://github.com/SLINGhub/RFkit/releases/latest) page. 

RFkit is a freely distributed software package for processing targeted metabolomics data from RapidFire-MS experiments.

## What it does

The tool takes the following as input: (i) mzML files containing the data for individual plates, (ii) transition information and (iii) batch log file containing sipping time information from the RapidFire system. The package automatically splices the data into individual injections and reports the total ion intensities of transitions integrated over the analysis time, with edges trimmed to yield expected peak shapes.


## Building from source

Once the repository is downloaded:

Windows:
```powershell
cd RFkit
powershell -ExecutionPolicy Bypass -File gui/build-windows.ps1
```

The script creates a lightweight per-user setup executable under
`gui/src-tauri/target/release/bundle/nsis`. It bundles the RFkit processing
engine, so the installed GUI does not require Rust, R, or any source files.
The setup program uses the WebView2 runtime already present on current Windows
systems. If WebView2 is missing, setup downloads and installs Microsoft's small
bootstrapper at that point instead of making every RFkit download include the
full runtime. Windows then keeps the shared runtime updated normally.

MacOS:
```bash
cd RFkit
bash gui/build-macos.sh
```

The default command creates a native, ad-hoc signed app and DMG for local
testing. The finished files are under `gui/src-tauri/target/release/bundle`.

For a public macOS release, install a **Developer ID Application** certificate
and its private key in the login keychain. Then run:

```bash
bash gui/build-macos.sh --release --universal
```

To use a `notarytool` Keychain profile instead of environment variables:

```bash
bash gui/build-macos.sh --release --universal --notary-profile rfkit-notary
```

The release build is signed with the Developer ID identity, uses the hardened
runtime, is submitted to Apple's notary service, has the notarization ticket
stapled to the app, and is checked with Gatekeeper. The script prompts for the
Apple Account email, Team ID, and an app-specific password when notarization
environment variables are not already configured. The universal option creates
one DMG that runs natively on both Apple Silicon and Intel Macs.

Use a DMG for normal drag-to-Applications distribution. A PKG additionally
needs a Developer ID Installer certificate and is only useful when RFkit needs
installer actions outside the app bundle.
