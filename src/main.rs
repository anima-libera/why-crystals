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
/// [the Dwarf Fortress wiki tilset repo](https://dwarffortresswiki.org/Tileset_repository).
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

	fn draw_char<T: RenderTarget>(
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

pub fn main() {
	let sdl_context = sdl2::init().unwrap();
	let video_subsystem = sdl_context.video().unwrap();
	let _sdl_image_context = sdl2::image::init(sdl2::image::InitFlag::all()).unwrap();

	let mut window_canvas = video_subsystem
		.window("Why Crystals ?", 1200, 800)
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

	// For now you can get this from
	// [the Dwarf Fortress wiki tilset repo](https://dwarffortresswiki.org/Tileset_repository).
	let char_sprite_sheet_filepath = "local/Pastiche_8x8.png";
	let mut char_sprite_sheet =
		CharSpriteSheet::from_filepath(char_sprite_sheet_filepath, (8, 8), &texture_creator);

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

		window_canvas.set_draw_color(Color::RGB(5, 30, 25));
		window_canvas.clear();

		for (i, c) in "abcdefghijklmnopqrstuvwxyz".chars().enumerate() {
			char_sprite_sheet.draw_char(
				c as u32,
				&mut window_canvas,
				Color::RGB(
					(i * 31 % 255) as u8,
					((i + 90) * 47 % 255) as u8,
					(((i * 11) ^ (i * 17)) % 255) as u8,
				),
				Rect::new(100 + i as i32 * 32, 100, 32, 32),
			);
		}

		window_canvas.present();
	}
}
