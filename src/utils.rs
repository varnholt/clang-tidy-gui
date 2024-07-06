use std::fs::File;
use std::path::Path;
use std::path::PathBuf;
use std::io::Write;
use std::process::{Command, ExitStatus};
use std::thread;
use std::time::Duration;
use crate::config::{Config, Fix};


pub fn process_fixes(config: Config)
{
    // simulate a long-running task
    //thread::spawn(|| {
    //    std::thread::sleep(std::time::Duration::from_secs(5));

        for fix in &config.fixes {

            if (!fix.enabled)
            {
                continue;
            }

            println!("applying fix: {}", fix.name);

            // write clang tidy config file to project dir
            let checks = [&fix.name];
            write_clang_tidy_file(&config.project_path, &checks);

            // start run-clang-tidy
            let args = [&config.run_clang_tidy_path, ".", "-fix"];
            run_process_and_wait("python", &args, &config.project_path);
        }

        println!("done!");
    //});
}


fn run_process_and_wait(command: &str, args: &[&str], working_directory : &str)  {
    println!("executing {} {}", command, args.join(" "));

    let mut output = Command::new(command)
        .args(args)
        .current_dir(working_directory)
        .output()
        .expect("failed to execute process");

    println!("Output: {:?}", output);
}


/// Writes a .clang-tidy file with the specified clang-tidy checks.
///
/// # Arguments
///
/// * `path` - The path where the .clang-tidy file should be created.
/// * `checks` - A list of clang-tidy checks to be enabled.
fn write_clang_tidy_file<P: AsRef<Path>>(path: P, checks: &[&String]) -> std::io::Result<()> {
    // Create or open the file at the specified path
    let project_path = path.as_ref();
    let clang_tidy_file_path = project_path.join(".clang-tidy");
    let mut file = File::create(&clang_tidy_file_path)?;

    // Construct the checks string
    let checks_str = format!("-*{}", checks.iter().map(|&check| format!(",{}", check)).collect::<String>());

    // Define the contents of the .clang-tidy file
    let contents = format!(
        r#"Checks: '{}'
WarningsAsErrors: ''
HeaderFilterRegex: ''
AnalyzeTemporaryDtors: false
FormatStyle: none
User: ''
ExtraArgs: []
ExtraArgsBefore: []
"#,
        checks_str
    );

    // Write the contents to the file
    file.write_all(contents.as_bytes())?;

    println!("wrote {}", clang_tidy_file_path.display());

    Ok(())
}

//
// fn run_clang_tidy() -> std::io::Result<()> {
//     // Specify the path to the .clang-tidy file
//     let file_path = "path/to/.clang-tidy";
//
//     // Specify the list of clang-tidy checks to enable
//     let checks = vec![
//         "modernize-use-nullptr",
//         "modernize-loop-convert",
//         "modernize-use-auto",
//     ];
//
//     // Write the .clang-tidy file with the specified checks
//     write_clang_tidy_file(file_path, &checks)?;
//
//     println!("Successfully wrote .clang-tidy file at {}", file_path);
//
//     Ok(())
// }
