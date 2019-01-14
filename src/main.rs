use std::env::{args, current_exe};
use std::error::Error;
use std::ffi::OsStr;
use std::iter::once;
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::process::{exit, id, Command};
use std::ptr::null_mut;
use url::percent_encoding::{percent_decode, utf8_percent_encode, PATH_SEGMENT_ENCODE_SET};
use winapi::um::handleapi::CloseHandle;
use winapi::um::processthreadsapi::GetExitCodeProcess;
use winapi::um::shellapi::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
use winapi::um::synchapi::WaitForSingleObject;
use winapi::um::winbase::INFINITE;
use winapi::um::wincon::{AttachConsole, FreeConsole};
use winapi::um::winuser::SW_HIDE;

fn wstr<S: AsRef<OsStr> + ?Sized>(s: &S) -> Vec<u16> {
	OsStr::new(s).encode_wide().chain(once(0)).collect()
}

fn main() -> Result<(), Box<dyn Error>> {
	let args: Vec<String> = args().collect();

	if args.len() <= 1 || args[1] == "--help" {
		println!("Usage: sudo <COMMAND>...");
		println!("Run a command with administrator privileges.");
		exit(1);
	} else if args[1] == "--attach-id" {
		unsafe {
			FreeConsole();
			AttachConsole(args[2].parse()?);
		}
		let commands: Vec<String> = args
			.iter()
			.skip(3)
			.map(|v| String::from(percent_decode(v.as_bytes()).decode_utf8().unwrap()))
			.collect();
		match Command::new(&commands[0]).args(&commands[1..]).spawn() {
			Ok(mut child) => {
				let status = child.wait()?;
				exit(status.code().unwrap_or(9009))
			}
			Err(error) => {
				eprintln!("{}", error);
				exit(9009);
			}
		};
	} else {
		let verb = wstr("runas");
		let file = wstr(current_exe()?.as_os_str());
		let encoded: Vec<String> = args
			.iter()
			.skip(if args[1] == "--" { 2 } else { 1 })
			.map(|v| utf8_percent_encode(v, PATH_SEGMENT_ENCODE_SET).collect())
			.collect();
		let params = wstr(&(format!("--attach-id {} {}", id(), encoded.join(" "))));
		let mut info = SHELLEXECUTEINFOW {
			cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
			fMask: SEE_MASK_NOCLOSEPROCESS,
			hwnd: null_mut(),
			lpVerb: verb.as_ptr(),
			lpFile: file.as_ptr(),
			lpParameters: params.as_ptr(),
			lpDirectory: null_mut(),
			nShow: SW_HIDE,
			hInstApp: null_mut(),
			lpIDList: null_mut(),
			lpClass: null_mut(),
			hkeyClass: null_mut(),
			dwHotKey: 0,
			hMonitor: null_mut(),
			hProcess: null_mut(),
		};
		let mut code: u32 = 0;
		unsafe {
			if ShellExecuteExW(&mut info) == 0 {
				panic!("ShellExecuteExW failed");
			}
			WaitForSingleObject(info.hProcess, INFINITE);
			GetExitCodeProcess(info.hProcess, &mut code);
			CloseHandle(info.hProcess);
		}
		exit(code as i32);
	}
}
