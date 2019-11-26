#include "stdafx.h"

using tchar = wchar_t;
#define T_ L""
using tstring = basic_string<tchar>;
using tstring_view = basic_string_view<tchar>;

tstring replace_all(tstring str, const tstring& pattern, const tstring& subst) {
	for (size_t i = 0; (i = str.find(pattern, i)) != tstring::npos;) {
		str.replace(i, pattern.length(), subst);
		i += subst.length();
	}
	return str;
}

class OptionParser {
	const tchar* ptr;

	void trim_left() { while (*ptr == ' ' || *ptr == '\t') ++ptr; }
public:
	OptionParser(const tchar* ptr = GetCommandLineW()) : ptr(ptr) {}

	void skip_current_exe() {
		auto i = 0u;
		if (*ptr == '"') {
			++ptr;
			while (*ptr && *ptr != '"') ++ptr;
			if (*ptr) ++ptr;
		} else if (*ptr && *ptr > ' ') {
			while (*ptr && *ptr > ' ') ++ptr;
		}
	}

	tstring_view opt() {
		trim_left();
		if (*ptr != '-') return {};
		auto p0 = ptr;
		while (*ptr && *ptr != ' ' && *ptr != '\t') ++ptr;
		return tstring_view(p0, ptr - p0);
	}

	tstring_view arg() {
		trim_left();
		auto p0 = ptr;
		bool quoted = false;
		while (*ptr && (quoted || *ptr != ' ' && *ptr != '\t')) {
			if (*ptr == '"') quoted = !quoted;
			++ptr;
		}
		return tstring_view(p0, ptr - p0);
	}

	tstring_view remaining() { trim_left();	return ptr; }

	static tstring unquote(tstring_view str) {
		if (!str.empty() && str.front() == '"') str.remove_prefix(1);
		if (!str.empty() && str.back() == '"') str.remove_suffix(1);
		return replace_all(tstring(str), T_"\"\"", T_"\"");
	}
};

int help() {
	fputs("Usage: sudo <COMMAND>...\n", stderr);
	fputs("Run a command with administrator privileges.\n", stderr);
	return 1;
}

void check_error(bool success) {
	if (success) return;
	throw system_error(GetLastError(), system_category());
}

int main() try {
	OptionParser opt_parser;
	opt_parser.skip_current_exe();

	bool exec_attach = false;

	for (;;) {
		auto opt = opt_parser.opt();
		if (opt == T_"-h" || opt == T_"--help") {
			return help();
		} else if (opt == T_"--exec-attach") {
			exec_attach = true;
		} else if (opt == T_"--title") {
			SetConsoleTitleW(opt_parser.unquote(opt_parser.arg()).data());
		} else if (opt.empty() || opt == T_"--") {
			break;
		} else {
			fwprintf(stderr, T_"unrecognized option '%s'\n",
				tstring(opt).data());
			return 1;
		}
	}

	auto cmd = opt_parser.remaining();
	if (cmd.empty()) return help();

	HANDLE handle;

	if (exec_attach) {
		FreeConsole();
		AttachConsole(ATTACH_PARENT_PROCESS);

		STARTUPINFOW startup_info = { sizeof(startup_info) };
		PROCESS_INFORMATION proc_info;
		tstring cmd_mut{ cmd };
		check_error(CreateProcessW(NULL, cmd_mut.data(), NULL, NULL, false,
			NULL, NULL, NULL, &startup_info, &proc_info));
		CloseHandle(proc_info.hThread);
		handle = proc_info.hProcess;
	} else {
		SHELLEXECUTEINFOW info = { sizeof(info) };
		info.fMask = SEE_MASK_NOCLOSEPROCESS;
		info.lpVerb = L"runas";

		tstring exe_buf(MAX_PATH + 1, L'\0');
		for (;;) {
			auto size = GetModuleFileNameW(NULL, exe_buf.data(),
				(DWORD)exe_buf.size());
			if (size < exe_buf.size()) break;
			exe_buf.resize(exe_buf.size() * 2);
		}
		info.lpFile = exe_buf.data();
		tstring params = T_"--exec-attach "s + tstring(cmd);
		info.lpParameters = params.data();
		info.nShow = SW_HIDE;
		check_error(ShellExecuteExW(&info));
		handle = info.hProcess;
	}
	SetConsoleCtrlHandler(NULL, TRUE);

	DWORD code;
	if (!handle) return 9009;
	WaitForSingleObject(handle, INFINITE);
	GetExitCodeProcess(handle, &code);
	CloseHandle(handle);
	return code;

} catch (exception& e) {
	// stderr is invalid after AttachConsole
	auto err = _fdopen(_open_osfhandle(
		(intptr_t)GetStdHandle(STD_ERROR_HANDLE), 0), "w");
	fprintf(err, "%s\n", e.what());
	fclose(err);
	return 9009;
}
