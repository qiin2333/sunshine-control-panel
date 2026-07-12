#!/usr/bin/env python3
"""Generate the updater RTS-style construction GIF concept asset.

This script keeps the GIF reproducible while the updater window code owns the
surrounding copy and layout.
"""

from __future__ import annotations

import argparse
from pathlib import Path

from PIL import Image, ImageDraw


SIZE = (560, 240)
FRAME_COUNT = 24
FRAME_DURATION_MS = 80

COLORS = {
    "shadow": (54, 59, 70, 210),
    "ground": (88, 86, 80, 255),
    "grid": (77, 104, 94, 255),
    "cyan": (57, 197, 187, 255),
    "cyan2": (110, 231, 223, 255),
    "green": (92, 230, 125, 255),
    "green_dim": (64, 140, 82, 210),
    "yellow": (255, 207, 93, 255),
    "orange": (232, 139, 72, 255),
    "pink": (255, 140, 203, 255),
    "base": (105, 151, 131, 255),
    "base2": (143, 184, 162, 255),
    "base3": (72, 101, 92, 255),
    "dark": (42, 44, 48, 255),
    "track": (70, 74, 80, 255),
    "window": (139, 213, 216, 255),
    "metal": (160, 168, 174, 255),
    "paper": (255, 245, 221, 255),
    "ink": (48, 48, 59, 255),
}


def color(name: str) -> tuple[int, int, int, int]:
    return COLORS[name]


def rect(draw: ImageDraw.ImageDraw, xy: list[int], fill: str | tuple[int, int, int, int]) -> None:
    draw.rectangle(xy, fill=color(fill) if isinstance(fill, str) else fill)


def line(draw: ImageDraw.ImageDraw, xy: list[int], fill: str, width: int = 1) -> None:
    draw.line(xy, fill=color(fill), width=width)


def sparkle(draw: ImageDraw.ImageDraw, x: int, y: int, fill: str = "yellow") -> None:
    rect(draw, [x, y + 4, x + 12, y + 7], fill)
    rect(draw, [x + 4, y, x + 7, y + 12], fill)


def resolve_paths(output_dir: Path | None) -> tuple[Path, Path, Path]:
    script = Path(__file__).resolve()
    gui_root = script.parents[1]
    sunshine_root = script.parents[4]
    sprite = gui_root / "src-tauri" / "assets" / "updater-helper-sun-girl.png"
    return sunshine_root, sprite, output_dir or sunshine_root / "output"


def draw_sun_girl(
    draw: ImageDraw.ImageDraw,
    canvas: Image.Image,
    sprite: Image.Image,
    frame: int,
) -> None:
    sx = 106 + (frame % 3 - 1)
    sy = 108 + (-1 if frame % 6 in (1, 2) else 0)
    rect(draw, [sx - 7, sy + 58, sx + 92, sy + 68], "shadow")
    canvas.alpha_composite(sprite, (sx, sy))

    if frame >= 2:
        beam_y = sy + 38
        for x in range(sx + 88, 250, 12):
            if (x // 12 + frame) % 2 == 0:
                rect(draw, [x, beam_y, x + 6, beam_y + 3], "cyan")


def draw_ground(draw: ImageDraw.ImageDraw, frame: int) -> None:
    rect(draw, [96, 176, 458, 180], "ground")
    for index in range(8):
        x = 116 + index * 38
        fill = "cyan" if (index + frame // 2) % 4 == 0 else (150, 150, 150, 255)
        rect(draw, [x, 168, x + 10, 172], fill)


def draw_site_grid(draw: ImageDraw.ImageDraw, frame: int) -> None:
    x0, y0 = 248, 152
    width, height = 210, 44
    rect(draw, [x0 + 8, y0 + 30, x0 + width + 10, y0 + 42], "shadow")
    rect(draw, [x0, y0, x0 + width, y0 + height], "dark")

    for x in range(x0 + 8, x0 + width - 8, 18):
        rect(draw, [x, y0 + 8, x + 10, y0 + 12], "grid")
    for y in range(y0 + 18, y0 + height - 6, 10):
        line(draw, [x0 + 8, y, x0 + width - 8, y], "grid")

    progress = min(1.0, frame / (FRAME_COUNT - 1))
    rect(draw, [x0 + 22, y0 + height - 9, x0 + width - 22, y0 + height - 5], "grid")
    rect(
        draw,
        [
            x0 + 22,
            y0 + height - 9,
            int(x0 + 22 + (width - 44) * progress),
            y0 + height - 5,
        ],
        "cyan",
    )


def draw_wireframe(draw: ImageDraw.ImageDraw, frame: int) -> None:
    if frame < 3:
        return

    x, y = 270, 82
    height = min(70, (frame - 2) * 5)
    top = y + (70 - height)
    bright = "green" if frame % 2 == 0 else "cyan2"

    line(draw, [x, y + 70, x, top], bright, 2)
    line(draw, [x + 156, y + 70, x + 156, top], bright, 2)
    line(draw, [x, top, x + 156, top], bright, 2)
    line(draw, [x, y + 70, x + 156, y + 70], bright, 2)

    for offset in range(0, 156, 26):
        rib_top = max(top, y + 70 - height + offset // 4)
        line(draw, [x + offset, y + 70, x + offset + 18, rib_top], "green_dim")

    scan_y = top + ((frame * 7) % max(1, y + 70 - top))
    rect(draw, [x - 6, scan_y, x + 162, scan_y + 3], "green")


def draw_building(draw: ImageDraw.ImageDraw, frame: int) -> None:
    x, y = 270, 82
    level = min(5, max(0, (frame - 4) // 4 + 1))

    if level >= 1:
        rect(draw, [x + 8, y + 60, x + 164, y + 78], "dark")
        for index in range(5):
            wheel_x = x + 20 + index * 27
            rect(draw, [wheel_x, y + 65, wheel_x + 16, y + 74], "track")
            rect(draw, [wheel_x + 5, y + 68, wheel_x + 11, y + 72], "ground")
    if level >= 2:
        rect(draw, [x + 18, y + 34, x + 150, y + 62], "base")
        rect(draw, [x + 30, y + 42, x + 84, y + 55], "base2")
        rect(draw, [x + 92, y + 42, x + 138, y + 55], "base2")
        rect(draw, [x + 96, y + 45, x + 126, y + 51], "cyan")
    if level >= 3:
        rect(draw, [x + 40, y + 12, x + 96, y + 36], "base3")
        rect(draw, [x + 50, y + 20, x + 86, y + 29], "window")
        rect(draw, [x + 110, y + 18, x + 150, y + 36], "base3")
        rect(draw, [x + 118, y + 24, x + 140, y + 30], "yellow")
    if level >= 4:
        rect(draw, [x + 152, y - 4, x + 157, y + 20], "base3")
        rect(draw, [x + 142, y - 8, x + 168, y - 4], "yellow")
        rect(draw, [x + 20, y + 18, x + 32, y + 30], "pink")
    if level >= 5:
        sparkle(draw, x + 174, y - 14, "cyan")
        sparkle(draw, x + 8, y + 22, "yellow")
        rect(draw, [x + 118, y + 64, x + 150, y + 68], "cyan")


def draw_crane(draw: ImageDraw.ImageDraw, frame: int) -> None:
    if frame < 5 or frame > 18:
        return

    x, y = 236, 66
    rect(draw, [x, y + 68, x + 5, y + 112], "orange")
    rect(draw, [x, y + 68, x + 84, y + 72], "orange")
    hook_x = x + 30 + (frame * 5 % 46)
    line(draw, [hook_x, y + 72, hook_x, y + 92], "metal")
    rect(draw, [hook_x - 5, y + 92, hook_x + 5, y + 98], "yellow")


def draw_vfx(draw: ImageDraw.ImageDraw, frame: int) -> None:
    if frame in (6, 10, 14, 19):
        sparkle(draw, 292 + (frame * 7) % 80, 78 + (frame % 3) * 12, "yellow")
        sparkle(draw, 360 + (frame * 5) % 50, 92, "cyan")

    if 4 <= frame <= 12:
        for index in range(3):
            x = 282 + index * 48 + (frame % 2) * 3
            rect(draw, [x, 148 - index * 2, x + 10, 152 - index * 2], (190, 177, 132, 180))


def draw_frame(frame: int, sprite: Image.Image) -> Image.Image:
    canvas = Image.new("RGBA", SIZE, (0, 0, 0, 0))
    draw = ImageDraw.Draw(canvas)
    draw_ground(draw, frame)
    draw_sun_girl(draw, canvas, sprite, frame)
    draw_site_grid(draw, frame)
    draw_wireframe(draw, frame)
    draw_crane(draw, frame)
    draw_building(draw, frame)
    draw_vfx(draw, frame)
    return canvas


def make_checker_preview(frame: Image.Image) -> Image.Image:
    checker = Image.new("RGBA", SIZE, (0, 0, 0, 0))
    draw = ImageDraw.Draw(checker)
    for y in range(0, SIZE[1], 20):
        for x in range(0, SIZE[0], 20):
            fill = (231, 233, 237, 255) if ((x // 20 + y // 20) % 2 == 0) else (184, 190, 200, 255)
            draw.rectangle([x, y, x + 19, y + 19], fill=fill)
    checker.alpha_composite(frame, (0, 0))
    return checker


def save_outputs(frames: list[Image.Image], output_dir: Path, name: str) -> None:
    output_dir.mkdir(parents=True, exist_ok=True)
    gif_path = output_dir / f"{name}.gif"
    frames_path = output_dir / f"{name}-frames.png"
    sheet_path = output_dir / f"{name}-sheet.png"
    preview_path = output_dir / f"{name}-preview.png"
    checker_path = output_dir / f"{name}-checker-preview.png"

    frames[0].save(
        gif_path,
        save_all=True,
        append_images=frames[1:],
        duration=FRAME_DURATION_MS,
        loop=0,
        disposal=2,
    )

    full_sheet = Image.new("RGBA", (SIZE[0] * len(frames), SIZE[1]), (0, 0, 0, 0))
    for index, frame in enumerate(frames):
        full_sheet.alpha_composite(frame, (index * SIZE[0], 0))
    full_sheet.save(frames_path)

    sheet = Image.new("RGBA", (SIZE[0] * 6, SIZE[1]), (0, 0, 0, 0))
    for index, frame_no in enumerate([0, 4, 8, 12, 17, 23]):
        sheet.alpha_composite(frames[frame_no], (index * SIZE[0], 0))
    sheet.save(sheet_path)

    frames[12].save(preview_path)
    make_checker_preview(frames[12]).save(checker_path)

    print(gif_path)
    print(frames_path)
    print(sheet_path)
    print(preview_path)
    print(checker_path)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output-dir",
        type=Path,
        help="Directory for generated GIF and previews.",
    )
    parser.add_argument(
        "--name",
        default="sun-girl-rts-construction-effect",
        help="Output filename prefix.",
    )
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    sunshine_root, sprite_path, output_dir = resolve_paths(args.output_dir)
    sprite = Image.open(sprite_path).convert("RGBA")
    frames = [draw_frame(frame, sprite) for frame in range(FRAME_COUNT)]
    save_outputs(frames, output_dir or sunshine_root / "output", args.name)


if __name__ == "__main__":
    main()
