#![no_main]
#![no_std]

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;
use core::panic::PanicInfo;
use uefi::{prelude::*, boot::ScopedProtocol, Result};
use uefi::proto::console::gop::{BltOp, BltPixel, BltRegion, GraphicsOutput};
use uefi::proto::console::text::Key;

const FRAMERATE: usize = 30;
const FRAME_INTERVAL: usize = 1_000_000 / FRAMERATE;
const GRID_SIZE: usize = 12;
const BOMB_COUNT: usize = 50;
const GAME_OVER_DELAY: usize = 2_000_000;

const COLOR_HIDDEN: BltPixel = BltPixel::new(30, 30, 30);
const COLOR_REVEALED: BltPixel = BltPixel::new(100, 110, 120);
const COLOR_BOMB: BltPixel = BltPixel::new(20, 20, 20);
const COLOR_FLAG: BltPixel = BltPixel::new(200, 50, 50);
const COLOR_SELECTION: BltPixel = BltPixel::new(255, 255, 0);
const COLOR_BACKGROUND: BltPixel = BltPixel::new(10, 10, 10);
const COLOR_LOSE: BltPixel = BltPixel::new(150, 20, 20);
const COLOR_WIN: BltPixel = BltPixel::new(20, 150, 20);
const DOT_COLORS: [BltPixel; 9] = [
    COLOR_REVEALED,
    BltPixel::new(0, 100, 255),   
    BltPixel::new(0, 150, 0),     
    BltPixel::new(255, 0, 0),     
    BltPixel::new(0, 0, 150),     
    BltPixel::new(150, 0, 0),     
    BltPixel::new(0, 150, 150),   
    BltPixel::new(150, 0, 150),   
    BltPixel::new(100, 100, 100), 
];

#[global_allocator]
static GLOBAL_ALLOCATOR: uefi::allocator::Allocator = uefi::allocator::Allocator;

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Vec2 {
    x: usize,
    y: usize,
}

impl Vec2 {
    fn new(x: usize, y: usize) -> Self {
        Self { x, y }
    }
}

struct Buffer {
    width: usize,
    height: usize,
    pixels: Vec<BltPixel>,
}

impl Buffer {
    fn new(width: usize, height: usize) -> Self {
        Buffer {
            width,
            height,
            pixels: vec![BltPixel::new(0, 0, 0); width * height],
        }
    }

    fn pixel(&mut self, x: usize, y: usize) -> Option<&mut BltPixel> {
        if x < self.width && y < self.height {
            self.pixels.get_mut(y * self.width + x)
        } else {
            None
        }
    }

    fn blit(&self, gop: &mut ScopedProtocol<GraphicsOutput>) -> Result {
        gop.blt(BltOp::BufferToVideo {
            buffer: &self.pixels,
            src: BltRegion::Full,
            dest: (0, 0),
            dims: (self.width, self.height),
        })
    }

    fn fill(&mut self, color: BltPixel) {
        self.pixels.iter_mut().for_each(|p| *p = color);
    }

    fn draw_rect(&mut self, pos: Vec2, dims: Vec2, color: BltPixel) {
        for y in pos.y..(pos.y + dims.y) {
            for x in pos.x..(pos.x + dims.x) {
                if let Some(pixel) = self.pixel(x, y) {
                    *pixel = color;
                }
            }
        }
    }
}

struct Rng {
    seed: u64,
}

impl Rng {
    fn new(seed: u64) -> Self {
        Self { seed }
    }


    fn next(&mut self) -> u64 {
        let mut x = self.seed;
        // See: https://en.wikipedia.org/wiki/Xorshift
        // this is under the xorshift64 impl on that wikipedia page
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.seed = x;
        x
    }

    fn next_in_range(&mut self, range: core::ops::Range<usize>) -> usize {
        let start = range.start as u64;
        let end = range.end as u64;
        (start + self.next() % (end - start)) as usize
    }
}

#[derive(Clone, Copy, PartialEq)]
enum TileState {
    Hidden,
    Revealed,
    Flagged,
}

#[derive(Clone, Copy)]
struct Tile {
    state: TileState,
    is_bomb: bool,
    neighboring_bombs: u8,
}

// using AOS instead of SOA; not good for contiguous access but it should be fine
// also its a lot more readable and easier to debug stuff
impl Tile {
    fn new() -> Self {
        Self {
            state: TileState::Hidden,
            is_bomb: false,
            neighboring_bombs: 0,
        }
    }
}

enum GameStatus {
    Playing,
    Win,
    Lose,
}

struct Game {
    grid: Vec<Tile>,
    selection: Vec2,
    status: GameStatus,
    is_first_move: bool,
    rng: Rng,
}

impl Game {
    fn new(seed: u64) -> Self {
        Self {
            grid: vec![Tile::new(); GRID_SIZE * GRID_SIZE],
            selection: Vec2::new(GRID_SIZE / 2, GRID_SIZE / 2),
            status: GameStatus::Playing,
            is_first_move: true,
            rng: Rng::new(seed),
        }
    }

    fn tile_mut(&mut self, x: usize, y: usize) -> &mut Tile {
        &mut self.grid[y * GRID_SIZE + x]
    }

    fn tile(&self, x: usize, y: usize) -> &Tile {
        &self.grid[y * GRID_SIZE + x]
    }


    fn plant_bombs(&mut self, safe_pos: Vec2) {
        let mut safe_positions = Vec::new();
        safe_positions.push(safe_pos);
        
        for neighbor in self.get_neighbors(safe_pos.x, safe_pos.y) {
            safe_positions.push(neighbor);
        }

        let mut bombs_placed = 0;
        let mut attempts = 0;
        let max_attempts = BOMB_COUNT * 10;
        
        while bombs_placed < BOMB_COUNT && attempts < max_attempts {
            let x = self.rng.next_in_range(0..GRID_SIZE);
            let y = self.rng.next_in_range(0..GRID_SIZE);
            
            let is_safe = safe_positions.iter().any(|pos| pos.x == x && pos.y == y);
            
            if !is_safe && !self.tile(x, y).is_bomb {
                self.tile_mut(x, y).is_bomb = true;
                bombs_placed += 1;
            }
            attempts += 1;
        }
        
        for y in 0..GRID_SIZE {
            for x in 0..GRID_SIZE {
                if !self.tile(x, y).is_bomb {
                    let count = self.count_neighbor_bombs(x, y);
                    self.tile_mut(x, y).neighboring_bombs = count;
                }
            }
        }
    }

    fn get_neighbors(&self, x: usize, y: usize) -> Vec<Vec2> {
        let mut neighbors = Vec::new();
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let nx = x as isize + dx;
                let ny = y as isize + dy;
                if nx >= 0 && nx < GRID_SIZE as isize && ny >= 0 && ny < GRID_SIZE as isize {
                    neighbors.push(Vec2::new(nx as usize, ny as usize));
                }
            }
        }
        neighbors
    }

    fn count_neighbor_bombs(&self, x: usize, y: usize) -> u8 {
        let mut count = 0;
        for neighbor in self.get_neighbors(x, y) {
            if self.tile(neighbor.x, neighbor.y).is_bomb {
                count += 1;
            }
        }
        count
    }
    
    fn handle_input(&mut self, key_char: char) {
        if !matches!(self.status, GameStatus::Playing) {
            return;
        }
        match key_char {
            'w' => self.selection.y = self.selection.y.saturating_sub(1),
            's' => self.selection.y = (self.selection.y + 1).min(GRID_SIZE - 1),
            'a' => self.selection.x = self.selection.x.saturating_sub(1),
            'd' => self.selection.x = (self.selection.x + 1).min(GRID_SIZE - 1),
            'f' => self.toggle_flag(),
            't' => self.reveal_selected(),
            _ => {}
        }
    }

    fn toggle_flag(&mut self) {
        let tile = self.tile_mut(self.selection.x, self.selection.y);
        match tile.state {
            TileState::Hidden => tile.state = TileState::Flagged,
            TileState::Flagged => tile.state = TileState::Hidden,
            TileState::Revealed => {}
        }
    }

    fn reveal_selected(&mut self) {
        if self.is_first_move {
            self.plant_bombs(self.selection);
            self.is_first_move = false;
        }
        let sel_x = self.selection.x;
        let sel_y = self.selection.y;
        self.reveal_recursive(sel_x, sel_y);
        if self.tile(sel_x, sel_y).is_bomb && self.tile(sel_x, sel_y).state == TileState::Revealed {
            self.status = GameStatus::Lose;
            self.reveal_all_bombs();
        } else {
            self.check_win_condition();
        }
    }

    fn reveal_recursive(&mut self, x: usize, y: usize) {
        let tile = self.tile_mut(x, y);
        if tile.state != TileState::Hidden {
            return;
        }
        tile.state = TileState::Revealed;
        if tile.neighboring_bombs == 0 && !tile.is_bomb {
            for neighbor in self.get_neighbors(x, y) {
                self.reveal_recursive(neighbor.x, neighbor.y);
            }
        }
    }

    fn reveal_all_bombs(&mut self) {
        for tile in self.grid.iter_mut() {
            if tile.is_bomb {
                tile.state = TileState::Revealed;
            }
        }
    }

    fn check_win_condition(&mut self) {
        let hidden_tiles = self
            .grid
            .iter()
            .filter(|t| !t.is_bomb && t.state != TileState::Revealed)
            .count();
        if hidden_tiles == 0 {
            self.status = GameStatus::Win;
        }
    }
}

fn draw_game(game: &Game, buffer: &mut Buffer) {
    buffer.fill(COLOR_BACKGROUND);
    let (buffer_width, buffer_height) = (buffer.width, buffer.height);
    let smaller_dim = buffer_width.min(buffer_height);
    let tile_size = smaller_dim / GRID_SIZE;
    let grid_pixel_size = tile_size * GRID_SIZE;
    let offset_x = (buffer_width - grid_pixel_size) / 2;
    let offset_y = (buffer_height - grid_pixel_size) / 2;
    let tile_padding = tile_size / 10;

    for y in 0..GRID_SIZE {
        for x in 0..GRID_SIZE {
            let tile = game.tile(x, y);
            let pos = Vec2::new(offset_x + x * tile_size, offset_y + y * tile_size);
            let inner_pos = Vec2::new(pos.x + tile_padding, pos.y + tile_padding);
            let inner_dims = Vec2::new(tile_size - 2 * tile_padding, tile_size - 2 * tile_padding);
            let color = match tile.state {
                TileState::Hidden => COLOR_HIDDEN,
                TileState::Flagged => COLOR_HIDDEN,
                TileState::Revealed => {
                    if tile.is_bomb {
                        COLOR_BOMB
                    } else {
                        COLOR_REVEALED
                    }
                }
            };
            buffer.draw_rect(inner_pos, inner_dims, color);

            if tile.state == TileState::Flagged {
                let flag_size = inner_dims.x / 2;
                let flag_pos = Vec2::new(
                    inner_pos.x + (inner_dims.x - flag_size) / 2,
                    inner_pos.y + (inner_dims.y - flag_size) / 2,
                );
                buffer.draw_rect(flag_pos, Vec2::new(flag_size, flag_size), COLOR_FLAG);
            }

            if tile.state == TileState::Revealed && !tile.is_bomb && tile.neighboring_bombs > 0 {
                let dot_color = DOT_COLORS[tile.neighboring_bombs as usize];
                let dot_size = (inner_dims.x / 5).max(1);
                let center_x = inner_pos.x + inner_dims.x / 2;
                let center_y = inner_pos.y + inner_dims.y / 2;

                let positions = match tile.neighboring_bombs {
                    1 => vec![Vec2::new(center_x - dot_size / 2, center_y - dot_size / 2)],
                    2 => vec![
                        Vec2::new(center_x - dot_size * 2, center_y - dot_size / 2),
                        Vec2::new(center_x + dot_size, center_y - dot_size / 2),
                    ],
                    3 => vec![
                        Vec2::new(center_x - dot_size * 2, center_y - dot_size * 2),
                        Vec2::new(center_x - dot_size / 2, center_y - dot_size / 2),
                        Vec2::new(center_x + dot_size, center_y + dot_size),
                    ],
                    // i got kinda lazy and bored having to figure out exactly where the rectangles go...
                    // lets hope the user doesn't get more than 4 bombs. if they do, im pretty sure
                    // just a big square is good enough to let them know there's a whole lotta bombs.
                    _ => {
                        let big_dot_size = dot_size * 2;
                        vec![Vec2::new(center_x - big_dot_size / 2, center_y - big_dot_size / 2)]
                    }
                };
                let dot_dims = match tile.neighboring_bombs {
                    _ if tile.neighboring_bombs >= 4 => Vec2::new(dot_size*2, dot_size*2),
                    _ => Vec2::new(dot_size, dot_size)
                };
                for pos in positions {
                    buffer.draw_rect(pos, dot_dims, dot_color);
                }
            }
        }
    }

    let sel_pos = Vec2::new(
        offset_x + game.selection.x * tile_size,
        offset_y + game.selection.y * tile_size,
    );
    let thickness = (tile_padding / 2).max(1);

    buffer.draw_rect(sel_pos, Vec2::new(tile_size, thickness), COLOR_SELECTION);
    buffer.draw_rect(
        Vec2::new(sel_pos.x, sel_pos.y + tile_size - thickness),
        Vec2::new(tile_size, thickness),
        COLOR_SELECTION,
    );
    buffer.draw_rect(sel_pos, Vec2::new(thickness, tile_size), COLOR_SELECTION);
    buffer.draw_rect(
        Vec2::new(sel_pos.x + tile_size - thickness, sel_pos.y),
        Vec2::new(thickness, tile_size),
        COLOR_SELECTION,
    );
}

fn get_key_press() -> Option<char> {
    use uefi::Char16;

    match uefi::system::with_stdin(|stdin| stdin.read_key()) {
        // there is 100% a better way to do this
        Ok(Some(Key::Printable(key))) => {
            let w_lo = Char16::try_from('w').unwrap();
            let a_lo = Char16::try_from('a').unwrap();
            let s_lo = Char16::try_from('s').unwrap();
            let d_lo = Char16::try_from('d').unwrap();
            let f_lo = Char16::try_from('f').unwrap();
            let t_lo = Char16::try_from('t').unwrap();

            if key == w_lo {
                Some('w')
            } else if key == a_lo {
                Some('a')
            } else if key == s_lo {
                Some('s')
            } else if key == d_lo {
                Some('d')
            } else if key == f_lo {
                Some('f')
            } else if key == t_lo {
                Some('t')
            } else {
                None
            }
        }
        _ => None,
    }
}

fn get_random_seed() -> u64 {
    let initial_time = uefi::runtime::get_time().unwrap();
    return initial_time.nanosecond() as u64 + initial_time.second() as u64 * 1_000_000_000;
}


#[entry]
fn main() -> Status {
    uefi::helpers::init().unwrap();

    let gop_handle = boot::get_handle_for_protocol::<GraphicsOutput>().unwrap();
    let mut gop = boot::open_protocol_exclusive::<GraphicsOutput>(gop_handle).unwrap();

    let (width, height) = gop.current_mode_info().resolution();
    let mut buffer = Buffer::new(width, height);

    let mut game = Game::new(get_random_seed());

    loop {
        if let Some(key_char) = get_key_press() {
            game.handle_input(key_char);
        }

        match game.status {
            GameStatus::Playing => {
                draw_game(&game, &mut buffer);
            }
            GameStatus::Lose => {
                draw_game(&game, &mut buffer);
                buffer.blit(&mut gop).unwrap();
                boot::stall(GAME_OVER_DELAY);
                // nay!!
                buffer.fill(COLOR_LOSE);
                buffer.blit(&mut gop).unwrap();
                boot::stall(GAME_OVER_DELAY);

                game = Game::new(get_random_seed());
                continue;
            }
            GameStatus::Win => {
                draw_game(&game, &mut buffer);
                buffer.blit(&mut gop).unwrap();
                boot::stall(GAME_OVER_DELAY);
                // yay!!
                buffer.fill(COLOR_WIN);
                buffer.blit(&mut gop).unwrap();
                boot::stall(GAME_OVER_DELAY);
                game = Game::new(get_random_seed());
                continue;
            }
        }

        buffer.blit(&mut gop).unwrap();
        boot::stall(FRAME_INTERVAL);
    }
}
