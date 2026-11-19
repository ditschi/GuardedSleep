# Logo generator

Python scripts that built the SVG set in `docs/brand/` (`gen.py` writes the SVGs one level up, `render.py` and `sheet.py` render preview PNGs, `weight.py` the wave weight study). It is a **design tool**: CI and `just` do not run it, and the committed SVGs are the source of truth. Edit the SVGs by hand for small changes, and use the generator for geometry changes (then review the diff of every file).

Dependencies (a scratch virtualenv is enough, nothing is added to the repository): `shapely` and `Pillow`. `gen.py` also needs `inkscape` to outline the wordmark text, and the renderers call `inkscape` for rasterising. Run the scripts from this directory.
