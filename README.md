# tui_games

A collection of terminal games playable in your terminal.

## Install

Prebuilt binaries (no Rust required):

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/JuanDGA/tui_games/releases/latest/download/tui_games-installer.sh | sh
```

Windows (PowerShell):

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/JuanDGA/tui_games/releases/latest/download/tui_games-installer.ps1 | iex"
```

The installer puts `tui_games` in `~/.cargo/bin` (or `%USERPROFILE%\.cargo\bin` on Windows). If that directory is not on your `PATH`, the script will tell you how to add it. It also installs `tui_games-update` for later upgrades.

From crates.io, if you have Rust:

```bash
cargo install tui_games
```

From a local checkout:

```bash
cargo install --path .
```

Then run:

```bash
tui_games
```

## Games

- **Snake** — classic snake; eat food and avoid hitting yourself
- **Tetris** — clear lines with falling tetrominoes; ←→ move, ↑ rotate, ↓ soft drop, space hard drop
- **Pong** — first to 11; choose vs CPU or vs Player. A uses W/S, B uses ↑/↓ (vs CPU, You get both)
- **Arkanoid** — break the wall; A/D or ←/→ hold to move; 3 lives

## Controls

| Context | Keys |
| --- | --- |
| Menu | ↑/↓ navigate, Enter select, Q quit |
| Ready | controls on a pause-style overlay; Enter start, or C vs CPU / F vs Player; M or Esc menu |
| In game | Esc pause (plus each game's own instructions) |
| Paused | R resume, M main menu |
| Game over | P play again, M main menu |

## License

MIT
