package argv

import "strings"

// MulticallBasename is the last path component of argv0, with a trailing .exe
// stripped so Windows and Unix agree.
func MulticallBasename(argv0 string) string {
	name := argv0
	if i := strings.LastIndexAny(name, `/\`); i >= 0 {
		name = name[i+1:]
	}
	if len(name) >= 4 && strings.EqualFold(name[len(name)-4:], ".exe") {
		return name[:len(name)-4]
	}
	return name
}

// AppletFromArgv0 is the applet name to parse as the first word, when argv0 is
// not the dispatcher. ok is false for a dispatcher invocation (busybox ls):
// skip argv0 and parse the rest. ok is true for a symlink invocation (ls -l):
// inject the basename.
func AppletFromArgv0(argv0, name, bin string) (applet string, ok bool) {
	base := MulticallBasename(argv0)
	if name != "" && base == MulticallBasename(name) {
		return "", false
	}
	if bin != "" && base == MulticallBasename(bin) {
		return "", false
	}
	return base, true
}

// RewriteMulticall prepends argv0's basename when it is an applet rather than
// the dispatcher. args are the tokens after the program name.
func RewriteMulticall(argv0 string, args []string, name, bin string) []string {
	applet, ok := AppletFromArgv0(argv0, name, bin)
	if !ok {
		return args
	}
	out := make([]string, 0, 1+len(args))
	return append(append(out, applet), args...)
}
