use std::env::current_exe;
use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::io;
use std::iter::once;
use std::mem::{size_of, zeroed};
use std::os::windows::prelude::*;
use std::process;
use std::process::exit;
use std::ptr::{null, null_mut};
use std::slice;
use winapi::um::handleapi::CloseHandle;
use winapi::um::processenv::GetCommandLineW;
use winapi::um::processthreadsapi::{
	CreateProcessW, GetExitCodeProcess, PROCESS_INFORMATION, STARTUPINFOW,
};
use winapi::um::shellapi::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
use winapi::um::synchapi::WaitForSingleObject;
use winapi::um::winbase::INFINITE;
use winapi::um::wincon::{AttachConsole, FreeConsole};
use winapi::um::winuser::SW_HIDE;

fn wstr<S: AsRef<OsStr> + ?Sized>(s: &S) -> Vec<u16> {
	OsStr::new(s).encode_wide().chain(once(0)).collect()
}

struct Opts {
	line: &'static [u16],
	exec: &'static [u16],
	help: bool,
	attach_id: Option<u32>,
}

const QUOTE: u16 = '"' as u16;
const SPACE: u16 = ' ' as u16;
const TAB: u16 = ' ' as u16;

fn index_of<T>(slice: &[T], pred: impl Fn(&T) -> bool, start: usize) -> usize {
	let mut i = start;
	while i < slice.len() && !pred(&slice[i]) {
		i += 1;
	}
	i
}

impl Opts {
	fn load() -> Opts {
		let line = unsafe {
			let ptr = GetCommandLineW();
			let mut size = 0;
			while *ptr.offset(size) != 0 {
				size += 1;
			}
			slice::from_raw_parts(ptr, size as usize)
		};
		Self {
			line,
			exec: &[],
			help: false,
			attach_id: None,
		}
	}

	fn skip_current_exe(&mut self) {
		if self.line.len() == 0 {
			return;
		}
		let mut i = match self.line[0] {
			QUOTE => index_of(self.line, |c| *c == QUOTE, 1),
			0...SPACE => 0,
			_ => index_of(self.line, |c| *c <= SPACE, 1),
		};
		if i < self.line.len() {
			i += 1;
		}
		self.line = &self.line[i..];
	}

	fn trim_left(slice: &[u16]) -> &[u16] {
		let i = index_of(slice, |c| *c != SPACE && *c != TAB, 0);
		&slice[i..]
	}

	fn parse_args(&mut self) -> Result<(), Box<dyn Error>> {
		loop {
			self.line = Self::trim_left(self.line);
			let i = index_of(self.line, |c| *c == SPACE || *c == TAB, 0);
			let opt_os_str = OsString::from_wide(&self.line[0..i]);
			let opt = opt_os_str.to_string_lossy();

			if opt.is_empty() || !opt.starts_with("-") {
				self.exec = self.line;
				return Ok(());
			}
			self.line = &self.line[i..];

			if opt == "--" {
				self.exec = Self::trim_left(self.line);
				return Ok(());
			} else if opt == "--help" || opt == "-h" {
				self.help = true;
				return Ok(());
			} else if opt == "--attach-id" {
				self.line = Self::trim_left(self.line);
				let i = index_of(self.line, |c| *c == SPACE || *c == TAB, 0);
				let id_str = OsString::from_wide(&self.line[0..i]);
				self.attach_id = Some(id_str.to_string_lossy().parse()?);
				self.exec = Self::trim_left(&self.line[i..]);
				return Ok(());
			} else {
				Err(io::Error::new(io::ErrorKind::Other, "Invalid option"))?;
			}
		}
	}
}

fn check_os_error(success: bool) {
	if !success {
		eprintln!("{}", io::Error::last_os_error());
		exit(9009);
	}
}

fn main() {
	let mut opts = Opts::load();
	opts.skip_current_exe();
	if let Err(error) = opts.parse_args() {
		eprintln!("{}", error);
		exit(1);
	}

	if opts.help || opts.exec.is_empty() {
		eprintln!("Usage: sudo <COMMAND>...");
		eprintln!("Run a command with administrator privileges.");
		exit(1);
	}

	let h_process = if let Some(attach_id) = opts.attach_id {
		let mut exec = opts.exec.to_owned();
		exec.push(0);
		unsafe {
			FreeConsole();
			AttachConsole(attach_id);

			let mut startup_info: STARTUPINFOW = zeroed();
			startup_info.cb = size_of::<STARTUPINFOW>() as u32;
			let mut process_information: PROCESS_INFORMATION = zeroed();
			check_os_error(
				CreateProcessW(
					null(),
					exec.as_mut_ptr(),
					null_mut(),
					null_mut(),
					0,
					0,
					null_mut(),
					null_mut(),
					&mut startup_info,
					&mut process_information,
				) != 0,
			);
			CloseHandle(process_information.hThread);
			process_information.hProcess
		}
	} else {
		let verb = wstr("runas");
		let file = wstr(current_exe().unwrap().as_os_str());
		let mut params: Vec<u16> = OsStr::new(&(format!("--attach-id {} ", process::id())))
			.encode_wide()
			.collect();
		params.extend_from_slice(opts.exec);
		params.push(0);

		let mut info: SHELLEXECUTEINFOW;
		unsafe {
			info = zeroed();
		}
		info.cbSize = size_of::<SHELLEXECUTEINFOW>() as u32;
		info.fMask = SEE_MASK_NOCLOSEPROCESS;
		info.lpVerb = verb.as_ptr();
		info.lpFile = file.as_ptr();
		info.lpParameters = params.as_ptr();
		info.nShow = SW_HIDE;

		unsafe {
			check_os_error(ShellExecuteExW(&mut info) != 0);
			info.hProcess
		}
	};

	let mut code: u32 = 0;
	unsafe {
		WaitForSingleObject(h_process, INFINITE);
		GetExitCodeProcess(h_process, &mut code);
		CloseHandle(h_process);
	}
	exit(code as i32);
}
