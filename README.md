Minesweeper as a UEFI app, built using the Rust UEFI crate. Uses QEMU as the emulator.

https://github.com/user-attachments/assets/a06e56d5-2f4f-4fa5-9ceb-6c2aaf696a3d

Controls:  
WASD - Move tile selector  
F - Flag currently selected tile as bomb  
T - Test currently selected tile. If it is not a bomb, reveal the number of neighbours (includes diagonals; 0-8 inclusive) which are bombs; otherwise, you lose!

You are guaranteed to start on an empty tile. If an empty tile is revealed, it recursively reveals all neighbours if the current tile in the recursion stack is empty.  
There are currently 50 bombs in a 12x12 (=144) grid. If more than three neighbours are bombs, it defaults to a big square with varying colours depending on precisely how many neighbours are bombs (definitely not because I didn't want to have to code the position of each dot).

To run and compile, do `chmod +x compile.bash` (only once) followed by `./compile.bash`. The first build may take a minute or two.   
You may need to replace the OVMF files if you run something different to Ubuntu, which you can find in `/usr/share/OVMF/`
