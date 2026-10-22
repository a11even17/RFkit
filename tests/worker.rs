use base64::Engine;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static NEXT_PROJECT: AtomicU64 = AtomicU64::new(0);

struct Project(PathBuf);

impl Project {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "rfkit_worker_test_{}_{}",
            std::process::id(),
            NEXT_PROJECT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        fs::create_dir(path.join("mzml_dir")).unwrap();
        fs::write(
            path.join("param.txt"),
            "transition_list = 'transitions.csv'\noffset = 0.0\n",
        )
        .unwrap();
        fs::write(path.join("transitions.csv"), "ID,Q1,Q3\nTest,100,50\n").unwrap();
        fs::write(
            path.join("acq_time.csv"),
            "sequence1.mzML,,\n\"(1, 1)\",0,2\n",
        )
        .unwrap();
        Self(path)
    }

    fn run(&self, argument: Option<&str>) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_RFkit"));
        command
            .env("RFKIT_PROJECT_DIR", &self.0)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        if let Some(argument) = argument {
            command.arg(argument);
        }
        let mut child = command.spawn().unwrap();
        let started = Instant::now();
        while child.try_wait().unwrap().is_none() {
            if started.elapsed() > Duration::from_secs(5) {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("worker did not terminate for {argument:?}");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        child.wait_with_output().unwrap()
    }

    fn write_mzml(&self, xml: &str) {
        fs::write(self.0.join("mzml_dir/sequence1.mzML"), xml).unwrap();
    }

    fn assert_auc(&self) {
        let output = self.run(Some("2only"));
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let csv = fs::read_to_string(self.0.join("long.csv")).unwrap();
        assert!(csv.contains("1.000,600.0,10.0"), "{csv}");
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn array(values: &[f64], time: bool, compressed: bool, precision64: bool) -> String {
    let bytes = values
        .iter()
        .flat_map(|value| {
            if precision64 {
                value.to_le_bytes().to_vec()
            } else {
                (*value as f32).to_le_bytes().to_vec()
            }
        })
        .collect::<Vec<_>>();
    let bytes = if compressed {
        let mut encoder =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&bytes).unwrap();
        encoder.finish().unwrap()
    } else {
        bytes
    };
    format!(
        "<binaryDataArray><cvParam accession='{}'/><cvParam accession='{}'/><cvParam accession='{}'/><binary>{}</binary></binaryDataArray>",
        if precision64 {
            "MS:1000523"
        } else {
            "MS:1000521"
        },
        if compressed {
            "MS:1000574"
        } else {
            "MS:1000576"
        },
        if time { "MS:1000595" } else { "MS:1000515" },
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

fn mzml(indexed: bool, intensities: &[f64], compressed: bool, precision64: bool) -> String {
    let xml = format!(
        "<mzML><run><chromatogramList><chromatogram><precursor><isolationWindow><cvParam accession='MS:1000827' value='100'/></isolationWindow></precursor><product><isolationWindow><cvParam accession='MS:1000827' value='50'/></isolationWindow></product><binaryDataArrayList>{}{}</binaryDataArrayList></chromatogram></chromatogramList></run></mzML>",
        array(&[0.0, 1.0, 2.0], true, compressed, precision64),
        array(intensities, false, compressed, precision64)
    );
    if indexed {
        format!("<indexedmzML>{xml}</indexedmzML>")
    } else {
        xml
    }
}

#[test]
fn indexed_mzml_preserves_known_triangle_area() {
    for compressed in [false, true] {
        for precision64 in [false, true] {
            let project = Project::new();
            project.write_mzml(&mzml(true, &[0.0, 10.0, 0.0], compressed, precision64));
            project.assert_auc();
        }
    }
}

#[test]
fn ordinary_mzml_preserves_known_triangle_area() {
    let project = Project::new();
    project.write_mzml(&mzml(false, &[0.0, 10.0, 0.0], false, true));
    project.assert_auc();
}

#[test]
fn second_based_retention_times_match_minute_based_area() {
    let project = Project::new();
    let seconds = array(&[0.0, 60.0, 120.0], true, false, true).replace(
        "accession='MS:1000595'",
        "accession='MS:1000595' unitAccession='UO:0000010'",
    );
    let xml = mzml(true, &[0.0, 10.0, 0.0], false, true)
        .replace(&array(&[0.0, 1.0, 2.0], true, false, true), &seconds);
    project.write_mzml(&xml);
    project.assert_auc();
}

#[test]
fn integer_offset_accepted_by_gui_also_works_in_worker() {
    let project = Project::new();
    fs::write(
        project.0.join("param.txt"),
        "transition_list = 'transitions.csv'\noffset = 0\n",
    )
    .unwrap();
    project.write_mzml(&mzml(true, &[0.0, 10.0, 0.0], false, true));
    project.assert_auc();
}

#[test]
fn mismatched_chromatogram_arrays_fail_instead_of_truncating_results() {
    let project = Project::new();
    project.write_mzml(&mzml(true, &[0.0, 10.0], false, true));
    let output = project.run(Some("2only"));
    assert!(
        !output.status.success(),
        "mismatched arrays must not produce successful results"
    );
    assert!(!project.0.join("long.csv").exists());
}

#[test]
fn missing_run_element_returns_error_without_hanging() {
    let project = Project::new();
    project.write_mzml("<mzML/>");
    let output = project.run(Some("2only"));
    assert!(!output.status.success());
}

#[test]
fn interactive_worker_exits_when_stdin_closes() {
    let project = Project::new();
    assert!(project.run(None).status.success());
}

#[test]
fn acquisition_generation_feeds_area_calculation() {
    let project = Project::new();
    fs::write(project.0.join("batch.rftime"), "plate\tsip\tseq\trow\tcol\tsiptime\n------------------------------------------------\nPlate One\t1\t1\t1\t1\t0\nPlate One\t2\t1\t3001\t3\t120\n").unwrap();
    project.write_mzml(&mzml(true, &[0.0, 10.0, 0.0], false, true));
    assert!(project.run(Some("1")).status.success());
    let acquisition = fs::read_to_string(project.0.join("acq_time.csv")).unwrap();
    assert!(
        acquisition.contains("\"(1, 1)\",0.000,2.000"),
        "{acquisition}"
    );
    project.assert_auc();
}

#[test]
fn zero_numbered_sequence_is_not_silently_dropped() {
    let project = Project::new();
    fs::write(project.0.join("batch.rftime"), "plate\tsip\tseq\trow\tcol\tsiptime\n------------------------------------------------\nPlate Zero\t1\t0\t1\t1\t0\nPlate Zero\t2\t0\t3001\t3\t120\n").unwrap();
    assert!(project.run(Some("1")).status.success());
    let acquisition = fs::read_to_string(project.0.join("acq_time.csv")).unwrap();
    assert!(acquisition.contains("sequence0.mzML"), "{acquisition}");
    assert!(
        acquisition.contains("\"(1, 1)\",0.000,2.000"),
        "{acquisition}"
    );
}

#[test]
fn incomplete_batch_rows_report_error_without_panic() {
    let project = Project::new();
    fs::write(project.0.join("batch.rftime"), "plate\tsip\tseq\trow\tcol\tsiptime\n------------------------------------------------\nbroken\n").unwrap();
    let output = project.run(Some("1"));
    assert!(!output.status.success());
    assert!(!String::from_utf8_lossy(&output.stderr).contains("panicked"));
}

#[test]
fn malformed_batch_pairs_preserve_existing_acquisition_file() {
    let cases = [
        "Plate\t1\t1\t3001\t3\t120\n",
        "Plate\t1\t1\t1\t1\t0\n",
        "Plate\t1\t1\t1\t1\t0\nPlate\t2\t1\t2\t1\t60\n",
        "Plate\t1\t1\t1\t1\t0\nPlate\t2\t2\t3001\t3\t120\n",
        "Plate\t1\t1\t1\t1\t120\nPlate\t2\t1\t3001\t3\t0\n",
        "Plate\t1\t1\t1\t1\tNaN\nPlate\t2\t1\t3001\t3\t120\n",
    ];
    for rows in cases {
        let project = Project::new();
        let previous = fs::read(project.0.join("acq_time.csv")).unwrap();
        fs::write(
            project.0.join("batch.rftime"),
            format!("plate\tsip\tseq\trow\tcol\tsiptime\n---\n{rows}"),
        )
        .unwrap();
        let output = project.run(Some("1"));
        assert!(!output.status.success(), "{rows}");
        assert!(!String::from_utf8_lossy(&output.stderr).contains("panicked"));
        assert_eq!(fs::read(project.0.join("acq_time.csv")).unwrap(), previous);
    }
}

#[test]
fn invalid_mzml_arrays_fail_without_panic_or_results() {
    let valid = mzml(true, &[0.0, 10.0, 0.0], false, true);
    let mut incomplete_bytes = [0.0_f64, 1.0, 2.0]
        .into_iter()
        .flat_map(f64::to_le_bytes)
        .collect::<Vec<_>>();
    incomplete_bytes.push(1);
    let incomplete = base64::engine::general_purpose::STANDARD.encode(incomplete_bytes);
    let valid_bytes = base64::engine::general_purpose::STANDARD.encode(
        [0.0_f64, 1.0, 2.0]
            .into_iter()
            .flat_map(f64::to_le_bytes)
            .collect::<Vec<_>>(),
    );
    let cases = [
        valid.replace(&valid_bytes, &incomplete),
        valid.replace(
            &array(&[0.0, 1.0, 2.0], true, false, true),
            &array(&[0.0, 2.0, 1.0], true, false, true),
        ),
        mzml(true, &[0.0, f64::NAN, 0.0], false, true),
        valid.replace(&array(&[0.0, 1.0, 2.0], true, false, true), ""),
        valid.replace("</run>", ""),
    ];
    for xml in cases {
        let project = Project::new();
        project.write_mzml(&xml);
        let output = project.run(Some("2only"));
        assert!(!output.status.success());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("panicked"));
        assert!(!project.0.join("long.csv").exists());
    }
}

#[test]
fn malformed_acquisition_csv_returns_error_without_panic() {
    for csv in [
        "sequence1.mzML\n",
        "sequence1.mzML,,\nwell,invalid,2\n",
        "well,0,2\nsequence1.mzML,,\nwell,0,2\n",
        "sequence1.mzML,,\nwell,2,0\n",
        "sequence1.mzML,,\nwell,NaN,2\n",
        "",
    ] {
        let project = Project::new();
        project.write_mzml(&mzml(true, &[0.0, 10.0, 0.0], false, true));
        fs::write(project.0.join("acq_time.csv"), csv).unwrap();
        let output = project.run(Some("2only"));
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "accepted acquisition CSV: {csv}");
        assert!(!stderr.contains("panicked"), "{csv}: {stderr}");
        assert!(!project.0.join("long.csv").exists());
    }
}

#[test]
fn malformed_assay_csv_returns_error_without_panic() {
    for csv in [
        "ID,Q1\nTest,100\n",
        "ID,Q1,Q3\nTest,invalid,50\n",
        "ID,Q1,Q3\nTest,NaN,50\n",
        "ID,Q1,Q3\nTest,100\n",
        "ID,Q1,Q3\n",
    ] {
        let project = Project::new();
        project.write_mzml(&mzml(true, &[0.0, 10.0, 0.0], false, true));
        fs::write(project.0.join("transitions.csv"), csv).unwrap();
        let output = project.run(Some("2only"));
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "accepted assay CSV: {csv}");
        assert!(!stderr.contains("panicked"), "{csv}: {stderr}");
        assert!(!project.0.join("long.csv").exists());
    }
}

#[test]
fn missing_transition_returns_descriptive_error_without_panic() {
    let project = Project::new();
    project.write_mzml(&mzml(true, &[0.0, 10.0, 0.0], false, true));
    fs::write(
        project.0.join("transitions.csv"),
        "ID,Q1,Q3\nMissing,200,50\n",
    )
    .unwrap();
    let output = project.run(Some("2only"));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(stderr.contains("Missing"), "{stderr}");
    assert!(!stderr.contains("panicked"), "{stderr}");
}

#[test]
fn failed_sample_processing_preserves_existing_plot() {
    for invalid_mzml in [true, false] {
        let project = Project::new();
        let valid = mzml(true, &[0.0, 10.0, 0.0], false, true);
        project.write_mzml(if invalid_mzml { "<mzML/>" } else { &valid });
        if !invalid_mzml {
            fs::write(
                project.0.join("transitions.csv"),
                "ID,Q1,Q3\nTest,100,50\nMissing,200,50\n",
            )
            .unwrap();
        }
        fs::create_dir(project.0.join("misc")).unwrap();
        let plot = project.0.join("misc/plot_sequence1.mzML.bin");
        fs::write(&plot, b"previous valid plot").unwrap();
        let output = project.run(Some("2only"));
        assert!(!output.status.success());
        assert_eq!(fs::read(&plot).unwrap(), b"previous valid plot");
    }
}
