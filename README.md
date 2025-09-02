minesweeper as a uefi app; run using qemu on ubuntu linux. 

[Screencast from 2025-09-02 22-17-13.webm](https://github.com/user-attachments/assets/8b1ac8f0-ba37-48c3-abb6-a6f0e5b887a5)
as you can probably tell i am really bad at minesweeper


supports keyboard only; i don't know how to get mouse input in a uefi app.
Controls:
WASD: move up, down, left, right
E: Toggle Flag
Q: Reveal selected bomb. (if you reveal a tile that is actually a bomb, you lose!)

Made using Rust and the uefi crate; Neovim (astronvim) is my editor
