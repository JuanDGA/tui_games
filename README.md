# tui_games

A collection of terminal games playable in your terminal.

## Install

```bash
cargo install tui_games
```

Then run:

```bash
tui_games
```

From a local checkout, you can also install with `cargo install --path .`.

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
