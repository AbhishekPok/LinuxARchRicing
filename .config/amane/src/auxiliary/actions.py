"""Action worker. All user strings are argv, never shell commands.
No capture/power operation runs merely by opening an island.
"""
import json, os, tempfile, pathlib, shutil, signal, subprocess, sys, time, urllib.parse, threading, shlex

def tool(name):
    path = shutil.which(name)
    if not path:
        raise RuntimeError(f'{name} is not installed.')
    return path

def first_tool(*names):
    """First of several interchangeable tools that is actually installed.

    The upstream config is a NixOS one and names tools its module installs.
    On this machine some of those are absent and an equivalent is already
    present, so prefer whichever exists rather than hard-failing."""
    for name in names:
        path = shutil.which(name)
        if path:
            return path
    raise RuntimeError('none of these are installed: ' + ', '.join(names))

class Cancelled(Exception):
    pass

CURRENT = None
CANCELLED = False

def on_cancel(*_):
    # SIGTERM from the shell: stop whatever is waiting (usually slurp) and leave.
    global CANCELLED
    CANCELLED = True
    if CURRENT is not None and CURRENT.poll() is None:
        CURRENT.terminate()

def run(args, input=None, picking=False):
    """Runs a tool. stdin is never inherited: the shell keeps our stdin open as a
    control pipe, and slurp would wait on it forever to read predefined regions."""
    global CURRENT
    CURRENT = subprocess.Popen(args, stdin=subprocess.PIPE if input is not None else subprocess.DEVNULL,
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    out, err = CURRENT.communicate(input)
    code, CURRENT = CURRENT.returncode, None
    if CANCELLED:
        raise Cancelled()
    if code:
        msg = err.decode(errors='replace').strip()
        if picking and (not msg or 'cancel' in msg.lower()):
            raise Cancelled()
        raise RuntimeError(msg or f'{args[0]} failed')
    return out

def copy(path):
    # wl-copy forks a background clipboard server. It must not inherit our pipes,
    # or this call waits until the clipboard is overwritten and the UI never gets FILE.
    with open(path, 'rb') as data:
        result = subprocess.run([tool('wl-copy'), '--type', 'image/png'], stdin=data,
                                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    if result.returncode:
        raise RuntimeError('Could not copy image to clipboard')

def destination():
    d = pathlib.Path.home() / 'Pictures/screenshots'
    d.mkdir(parents=True, exist_ok=True)
    return d / (time.strftime('%Y-%m-%d_%H-%M-%S') + f'_{time.time_ns()%1000000:06d}' + '.png')

def scratch():
    # Unsaved captures live here until Save/Annotate; cancelling deletes them.
    base = pathlib.Path(os.environ.get('XDG_RUNTIME_DIR') or tempfile.gettempdir())
    return base / f'amane-shot-{time.time_ns()}.png'

def slurp_style(color):
    c = color.lstrip('#')[:6]
    # Fully transparent background, selection and option boxes: only the border
    # (in the theme colour) is drawn, so the screen stays visible underneath.
    return ['-b', '#00000000', '-s', '#00000000', '-B', '#00000000', '-c', f'#{c}ff', '-w', '3']

def geometry(kind, color):
    if kind == 'region':
        out = run([tool('slurp')] + slurp_style(color), picking=True).decode().strip()
        print('SELECTED', flush=True)
        return out
    if kind == 'window':
        # Real visible client rectangles, supplied to slurp's rectangle mode.
        # Hyprland port: geometry comes from hyprctl, and "visible" means the
        # window sits on its monitor's active workspace.
        clients = json.loads(run([tool('hyprctl'), '-j', 'clients']))
        monitors = json.loads(run([tool('hyprctl'), '-j', 'monitors']))
        visible = {(m.get('activeWorkspace') or {}).get('id') for m in monitors}
        rows = []
        for c in clients:
            if c.get('hidden') or not c.get('mapped', True):
                continue
            if ((c.get('workspace') or {}).get('id')) not in visible:
                continue
            (x, y), (w, h) = c.get('at', (0, 0)), c.get('size', (0, 0))
            if w > 0 and h > 0:
                rows.append(f'{int(x)},{int(y)} {int(w)}x{int(h)}')
        if not rows:
            raise RuntimeError('No visible windows available for capture.')
        out = run([tool('slurp'), '-r'] + slurp_style(color), input=('\n'.join(rows)+'\n').encode(), picking=True).decode().strip()
        print('SELECTED', flush=True)
        return out
    if kind != 'full':
        raise RuntimeError('Unknown capture type')
    return None

def focused_output():
    try:
        monitors = json.loads(run(['hyprctl', '-j', 'monitors']).decode())
        return next((m['name'] for m in monitors if m.get('focused')), None)
    except Exception:
        return None

def settle(started, minimum):
    # Let the island finish leaving, the border fade back and slurp unmap,
    # so none of them end up in the capture.
    time.sleep(max(0.0, minimum - (time.time() - started)))

def main():
    action, *args = sys.argv[1:]
    signal.signal(signal.SIGTERM, on_cancel)
    signal.signal(signal.SIGINT, on_cancel)
    started = time.time()
    # The capture colour always comes from the shell's sampled theme.
    color = args[1] if len(args) > 1 else None
    if action == 'screenshot' and not color:
        raise RuntimeError('No theme colour supplied')
    if action == 'screenshot':
        tool('grim');tool('wl-copy')
        if args[0] != 'full': tool('slurp')
        rect = geometry(args[0], color)
        settle(started, 1.0 if rect is None else 0.45)
        path=scratch()
        cmd=[tool('grim')]
        if rect: cmd += ['-g', rect]
        else:
            # "Screen" is the monitor you are on, as on macOS
            output = focused_output()
            if output: cmd += ['-o', output]
        cmd += [str(path)]
        run(cmd);print('FILE\t'+str(path), flush=True)
    elif action == 'save':
        tool('wl-copy')
        original=pathlib.Path(args[0]);target=destination()
        shutil.move(str(original),str(target));copy(target)
        print('DONE\t'+str(target), flush=True)
    elif action == 'edit':
        original=pathlib.Path(args[0]); output=destination()
        # Satty copies and saves when the user confirms. Closing it without
        # confirming leaves no output: that is a cancel, nothing is kept.
        try:
            annotator = first_tool('satty', 'swappy')
            if annotator.endswith('satty'):
                run([annotator,'--filename',str(original),'--output-filename',str(output),
                     '--copy-command',tool('wl-copy'),'--save-after-copy','--early-exit'])
            else:
                # swappy writes the result only when you save, so an unsaved
                # close still leaves no output file: the cancel check below
                # keeps working unchanged.
                run([annotator,'-f',str(original),'-o',str(output)])
        finally:
            original.unlink(missing_ok=True)
        if not output.exists():
            raise Cancelled()
        copy(output)
        print('DONE\t'+str(output), flush=True)
    elif action == 'browser':
        text=args[0].strip()
        if not text:raise RuntimeError('Type a search or URL after s ')
        # websearch.sh owns the engine prefixes (s gh ..., s aw ...) and brings
        # the browser forward afterwards
        script=pathlib.Path.home()/'.config/hypr/scripts/websearch.sh'
        if script.is_file():run([str(script),text])
        else:
            parsed=urllib.parse.urlsplit(text)
            if parsed.scheme in ('https','http'):url=text
            elif '.' in text and ' ' not in text:url='https://'+text
            else:url='https://www.google.com/search?q='+urllib.parse.quote(text)
            run([tool('xdg-open'),url])
        print('DONE\tOpened browser',flush=True)
    elif action == 'pick':
        # A launcher row: argv is the mode script with --menu-pick VALUE (or a
        # built-in action's script). OPEN lines ask the launcher to reopen.
        out=run(args).decode(errors='replace')
        for line in out.splitlines():
            if line.startswith('OPEN\t'):print(line,flush=True)
        print('DONE\tpicked',flush=True)
    elif action == 'launch':
        # freedesktop Exec uses quoted argv, not shell evaluation. The library
        # already removed field codes, so no query string ever becomes code.
        argv=shlex.split(args[0])
        if not argv:raise RuntimeError('Empty desktop command')
        subprocess.Popen(argv,stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,start_new_session=True)
        print('DONE\tApplication launched',flush=True)
    elif action == 'session':
        name=args[0]
        # hyprland.lua is Lua, so dispatch takes Lua: plain 'exit' is a syntax error
        if name=='logout':cmd=[tool('hyprctl'),'dispatch','hl.dsp.exit()']
        elif name in ('restart','shutdown','sleep'):
            cmd=[tool('systemctl'),{'restart':'reboot','shutdown':'poweroff','sleep':'suspend'}[name]]
        elif name=='lock':
            locker=first_tool('hyprlock','swaylock')
            script=pathlib.Path.home()/'.config/hypr/scripts/lock.sh'
            if script.is_file():
                # applies the active theme's palette, then runs hyprlock
                cmd=[str(script)]
            elif locker.endswith('hyprlock'):
                # hyprlock reads ~/.config/hypr/hyprlock.conf, which already
                # carries this machine's palette, so it needs no colour flags.
                cmd=[locker]
            else:
                text=json.loads((pathlib.Path.home()/'.local/state/theme/active.json').read_text())
                colors=text.get('colors',{})
                color=lambda key,fallback:colors.get(key,fallback).lstrip('#')[:6]
                cmd=[locker,'--color',color('BASE00','#000000'),'--indicator-radius','64',
                     '--ring-color',color('BLUE','#81a1c1'),'--inside-color',color('BASE00','#000000'),
                     '--line-color',color('BASE00','#000000'),'--text-color',color('WHITE','#d8dee9')]
        else:raise RuntimeError('Unknown session action')
        run(cmd);print('DONE\t'+name,flush=True)

if __name__=='__main__':
    try:main()
    except Cancelled:
        print('CANCEL', flush=True)
    except Exception as e:
        print('ERROR\t'+str(e).replace('\n',' ')[:350],flush=True)
        sys.exit(1)
