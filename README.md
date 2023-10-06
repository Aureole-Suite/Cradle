# Cradle
> Located on the south side of Liber Ark, the Cradle district is the cozy, beloved home of the majority of the Ark's citizens. It is comprised of 128 blocks, each with its own Halo Rail station, public service buildings, city offices, and event halls, allowing citizens to enjoy everything the Ark has to offer--all close to home! As there are vacancies in a third of the blocks due to recent population changes, feel free to inquire about owning another home at the nearest city office.

A converter between some of Falcom's file formats and more conventional ones.

Currently supported conversions:

- itp ↔ png
- itp ↔ dds

Planned features include support for itc, ch, chcp, and any other fun image formats Falcom may have cooked up.
Maybe also it3 and x2/x3 if I find a good format to convert that to/from.

## Usage

Simply drag the files to be converted onto the executable. Use `--help` on the commandline for more configuration options.

## Supported games

### itp
  - Trails in the Sky FC/SC/3rd (Xseed PC, PSP, Evolution)
  - Trails from Zero/to Azure (Geofront PC, NISA PC, PSP, Evolution)
  - The Legend of Nayuta: Boundless Trails (NISA PC, PSP)
  - Ys Seven (Xseed PC)
  - Ys VIII: Lacrimosa of Dana (NISA PC)
  - Ys vs Sora no Kiseki: Alternate Saga (PSP)

There exist hints that there exist ITP files with 16-bit color.
Unfortunately I have not found any such ITPs, so these formats are not implemented.
