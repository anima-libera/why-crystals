use sdl2::event::Event;
use sdl2::image::LoadSurface;
use sdl2::keyboard::Keycode;
use sdl2::pixels::{Color, PixelFormatEnum};
use sdl2::rect::Rect;
use sdl2::render::{BlendMode, Canvas, RenderTarget, Texture, TextureCreator};
use sdl2::surface::Surface;
use sdl2::video::WindowContext;

fn map_surface_pixels(surface: &Surface, mut f: impl FnMut(Color) -> Color) -> Surface<'static> {
	let mut new_surface = surface.convert_format(PixelFormatEnum::RGBA8888).unwrap();

	// From what I may have understood from posts on the Internet,
	// `SDL_ConvertSurfaceFormat` may decide to choose a format similar
	// to the one being requested if such format is not available
	// (because some pixel formats may not be available??), and also
	// the endianness may mess with our expectations of the order of bytes
	// in a pixel value (and on my machine it did be revered).
	// Thus, we will only assume that the format is of the size of a `u32`
	// and we also need transparency so it better be supported.
	let pixel_format_enum = new_surface.pixel_format_enum();
	assert!(pixel_format_enum.supports_alpha());
	assert!(pixel_format_enum.byte_size_per_pixel() == 4);

	let pixel_format = new_surface.pixel_format();
	let pitch = new_surface.pitch();
	let wh = (new_surface.width(), new_surface.height());
	new_surface.with_lock_mut(|pixels: &mut [u8]| {
		for y in 0..wh.1 {
			for x in 0..wh.0 {
				let index = (y * pitch + x * 4) as usize;
				let pixel = pixels[index..(index + 4)].as_mut_ptr() as *mut u32;

				// SAFETY: If this does not work then just go program something else.
				let old_color = unsafe { Color::from_u32(&pixel_format, *pixel) };
				let new_color = f(old_color);
				unsafe {
					*pixel = new_color.to_u32(&pixel_format);
				}
			}
		}
	});
	new_surface
}

/// Sprite sheet with ASCII-like sprites.
///
/// The order of the character sprites (left to right and top to bottom) is expected
/// to be the same as in [CP437](https://en.wikipedia.org/wiki/Code_page_437).
///
/// If the sheet texture is loaded from a file, such file can be obtained from
/// [the Dwarf Fortress wiki tileset repo](https://dwarffortresswiki.org/Tileset_repository).
struct CharSpriteSheet<'a> {
	texture: Texture<'a>,
	grid_wh: (u32, u32),
	tile_wh: (u32, u32),
}

impl<'a> CharSpriteSheet<'a> {
	fn from_filepath(
		filepath: &str,
		tile_wh: (u32, u32),
		texture_creator: &'a TextureCreator<WindowContext>,
	) -> CharSpriteSheet<'a> {
		let raw_surface = Surface::from_file(filepath).unwrap();
		let pink_and_black_to_transparent = |color| {
			if matches!(
				color,
				Color { r: 255, g: 0, b: 255, .. } | Color { r: 0, g: 0, b: 0, .. }
			) {
				Color::RGBA(0, 0, 0, 0)
			} else {
				color
			}
		};
		let surface = map_surface_pixels(&raw_surface, pink_and_black_to_transparent);
		let mut texture = texture_creator
			.create_texture_from_surface(surface)
			.unwrap();
		texture.set_blend_mode(BlendMode::Blend);
		CharSpriteSheet::from_texture(texture, tile_wh)
	}

	fn from_texture(texture: Texture<'a>, tile_wh: (u32, u32)) -> CharSpriteSheet {
		let texture_query = texture.query();
		let texture_wh = (texture_query.width, texture_query.height);
		assert!(texture_wh.0 % tile_wh.0 == 0);
		assert!(texture_wh.1 % tile_wh.1 == 0);
		let grid_wh = (texture_wh.0 / tile_wh.0, texture_wh.1 / tile_wh.1);
		CharSpriteSheet { texture, grid_wh, tile_wh }
	}

	fn char_index_to_rect(&self, char_index: u32) -> Rect {
		let grid_xy = (char_index % self.grid_wh.0, char_index / self.grid_wh.1);
		let xy = (grid_xy.0 * self.tile_wh.0, grid_xy.1 * self.tile_wh.1);
		Rect::new(xy.0 as i32, xy.1 as i32, self.tile_wh.0, self.tile_wh.1)
	}

	fn draw_char_to_canvas<T: RenderTarget>(
		&mut self,
		char_index: u32,
		canvas: &mut Canvas<T>,
		color: Color,
		dst: Rect,
	) {
		self.texture.set_color_mod(color.r, color.g, color.b);
		canvas
			.copy(&self.texture, self.char_index_to_rect(char_index), dst)
			.unwrap();
	}
}

type SpriteIndex = u32;

#[derive(Clone, Copy)]
struct ScreenTile {
	sprite: SpriteIndex,
	fg_color: Color,
	bg_color: Color,
}

const COLOR_WHITE: Color = Color { r: 180, g: 220, b: 200, a: 255 };
const COLOR_BG: Color = Color { r: 5, g: 30, b: 25, a: 255 };

impl ScreenTile {
	fn new() -> ScreenTile {
		ScreenTile {
			sprite: 0,
			fg_color: COLOR_WHITE,
			bg_color: COLOR_BG,
		}
	}

	fn from_char(character: char) -> ScreenTile {
		ScreenTile {
			sprite: character as SpriteIndex,
			fg_color: COLOR_WHITE,
			bg_color: COLOR_BG,
		}
	}
}

struct ScreenGrid {
	tiles: Vec<ScreenTile>,
	grid_wh: (u32, u32),
	tile_wh: (u32, u32),
}

impl ScreenGrid {
	fn new(grid_wh: (u32, u32), tile_wh: (u32, u32)) -> ScreenGrid {
		let tiles = std::iter::repeat(ScreenTile::new())
			.take((grid_wh.0 * grid_wh.1) as usize)
			.collect();
		ScreenGrid { tiles, grid_wh, tile_wh }
	}

	fn tile_index(&self, xy: (u32, u32)) -> usize {
		(xy.0 * self.grid_wh.1 + xy.1) as usize
	}

	fn tile(&self, xy: (u32, u32)) -> &ScreenTile {
		let tile_index = self.tile_index(xy);
		&self.tiles[tile_index]
	}

	fn tile_mut(&mut self, xy: (u32, u32)) -> &mut ScreenTile {
		let tile_index = self.tile_index(xy);
		&mut self.tiles[tile_index]
	}

	fn grid_coords_to_rect(&self, xy: (u32, u32)) -> Rect {
		Rect::new(
			(xy.0 * self.tile_wh.0) as i32,
			(xy.1 * self.tile_wh.1) as i32,
			self.tile_wh.0,
			self.tile_wh.1,
		)
	}

	fn draw_to_canvas<T: RenderTarget>(
		&self,
		canvas: &mut Canvas<T>,
		char_sprite_sheet: &mut CharSpriteSheet,
	) {
		for y in 0..self.grid_wh.1 {
			for x in 0..self.grid_wh.0 {
				let xy = (x, y);
				let dst = self.grid_coords_to_rect((x, y));

				// Fill the tile with the background.
				let bg_color = self.tile(xy).bg_color;
				canvas.set_draw_color(bg_color);
				canvas.fill_rect(dst).unwrap();

				// Draw the sprite after the background so that it is on the foreground.
				let sprite = self.tile(xy).sprite;
				let fg_color = self.tile(xy).fg_color;
				char_sprite_sheet.draw_char_to_canvas(sprite, canvas, fg_color, dst);
			}
		}
	}
}

#[derive(Clone, Copy)]
enum RichTextModifier {
	FgColor(Color),
	BgColor(Color),
}

#[derive(Clone)]
enum RichText {
	Text(String),
	Modifier(RichTextModifier, Box<RichText>),
	Sequence(Vec<RichText>),
}

impl<T> From<T> for RichText
where
	T: Into<String>,
{
	fn from(string: T) -> Self {
		RichText::Text(string.into())
	}
}

impl RichText {
	fn fg_color(self, color: Color) -> RichText {
		RichText::Modifier(RichTextModifier::FgColor(color), Box::new(self))
	}

	fn bg_color(self, color: Color) -> RichText {
		RichText::Modifier(RichTextModifier::BgColor(color), Box::new(self))
	}
}

impl std::ops::Add<RichText> for RichText {
	type Output = RichText;

	fn add(self, rhs: RichText) -> RichText {
		match self {
			RichText::Sequence(mut vec) => RichText::Sequence({
				vec.push(rhs);
				vec
			}),
			lhs => RichText::Sequence(vec![lhs, rhs]),
		}
	}
}

impl std::ops::AddAssign<RichText> for RichText {
	fn add_assign(&mut self, rhs: RichText) {
		match self {
			RichText::Sequence(ref mut vec) => vec.push(rhs),
			ref lhs => {
				*self = RichText::Sequence(vec![(*lhs).to_owned(), rhs]);
			},
		}
	}
}

impl RichText {
	fn tiles(&self) -> Vec<ScreenTile> {
		fn tiles_rec(
			formatted_text: &RichText,
			tiles: &mut Vec<ScreenTile>,
			modifiers: &mut Vec<RichTextModifier>,
		) {
			match formatted_text {
				RichText::Text(string) => {
					tiles.append(
						&mut string
							.chars()
							.map(|character| {
								let mut tile = ScreenTile::from_char(character);
								for modifier in modifiers.iter() {
									match *modifier {
										RichTextModifier::BgColor(bg_color) => {
											tile.bg_color = bg_color
										},
										RichTextModifier::FgColor(fg_color) => {
											tile.fg_color = fg_color
										},
									}
								}
								tile
							})
							.collect(),
					);
				},
				RichText::Modifier(modifier, sub_formatted_text) => {
					modifiers.push(*modifier);
					tiles_rec(&sub_formatted_text, tiles, modifiers);
					modifiers.pop();
				},
				RichText::Sequence(vec) => {
					for sub_formatted_text in vec.iter() {
						tiles_rec(&sub_formatted_text, tiles, modifiers);
					}
				},
			}
		}

		let mut tiles = Vec::new();
		let mut modifiers = Vec::new();
		tiles_rec(self, &mut tiles, &mut modifiers);
		tiles
	}
}

impl ScreenGrid {
	fn darw_text(&mut self, text: RichText, dst_xy: (u32, u32)) {
		for (i, formatted_tile) in text.tiles().iter().enumerate() {
			let tile = self.tile_mut((dst_xy.0 + i as u32, dst_xy.1));
			*tile = *formatted_tile;
		}
	}
}

pub fn main() {
	let sdl_context = sdl2::init().unwrap();
	let video_subsystem = sdl_context.video().unwrap();
	let _sdl_image_context = sdl2::image::init(sdl2::image::InitFlag::all()).unwrap();

	let mut window_canvas = video_subsystem
		.window("Why Crystals ?", 1200, 600)
		.position_centered()
		.maximized()
		.resizable()
		.build()
		.unwrap()
		.into_canvas()
		.present_vsync()
		.accelerated()
		.build()
		.unwrap();
	window_canvas.set_blend_mode(BlendMode::Blend);
	let texture_creator = window_canvas.texture_creator();

	// You can get more of these from
	// [the Dwarf Fortress wiki tileset repo](https://dwarffortresswiki.org/Tileset_repository).
	let char_sprite_sheet_filepath = "assets/Pastiche_8x8.png";
	let char_sprite_sheet_tile_wh = (8, 8);

	let mut char_sprite_sheet = CharSpriteSheet::from_filepath(
		char_sprite_sheet_filepath,
		char_sprite_sheet_tile_wh,
		&texture_creator,
	);

	let mut screen_grid = ScreenGrid::new((30, 30), (16, 16));

	screen_grid.darw_text("abcdefghijklmnopqrstuvwxyz".into(), (1, 1));
	screen_grid.darw_text(
		RichText::from("abcdef")
			+ RichText::from("ghijkl").fg_color(Color::RGB(240, 40, 5))
			+ RichText::from("mnopqr").bg_color(Color::RGB(10, 40, 150))
			+ RichText::from("stuvwx")
				.fg_color(Color::RGB(240, 40, 5))
				.bg_color(Color::RGB(10, 40, 150))
			+ (RichText::from("y") + RichText::from("z")).fg_color(Color::RGB(10, 210, 40)),
		(1, 2),
	);

	let mut event_pump = sdl_context.event_pump().unwrap();
	'gameloop: loop {
		for event in event_pump.poll_iter() {
			match event {
				Event::Quit { .. } | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
					break 'gameloop;
				},
				_ => {},
			}
		}

		window_canvas.set_draw_color(COLOR_BG);
		window_canvas.clear();

		screen_grid.draw_to_canvas(&mut window_canvas, &mut char_sprite_sheet);

		window_canvas.present();
	}
}
