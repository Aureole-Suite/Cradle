#[allow(non_snake_case)]
pub mod DDSD {
	pub const DEFAULT:         u32 = CAPS | HEIGHT | WIDTH | PIXELFORMAT;
	pub const CAPS:            u32 = 0x00000001;
	pub const HEIGHT:          u32 = 0x00000002;
	pub const WIDTH:           u32 = 0x00000004;
	pub const PITCH:           u32 = 0x00000008;
	pub const BACKBUFFERCOUNT: u32 = 0x00000020;
	pub const ZBUFFERBITDEPTH: u32 = 0x00000040;
	pub const ALPHABITDEPTH:   u32 = 0x00000080;
	pub const LPSURFACE:       u32 = 0x00000800;
	pub const PIXELFORMAT:     u32 = 0x00001000;
	pub const CKDESTOVERLAY:   u32 = 0x00002000;
	pub const CKDESTBLT:       u32 = 0x00004000;
	pub const CKSRCOVERLAY:    u32 = 0x00008000;
	pub const CKSRCBLT:        u32 = 0x00010000;
	pub const MIPMAPCOUNT:     u32 = 0x00020000;
	pub const REFRESHRATE:     u32 = 0x00040000;
	pub const LINEARSIZE:      u32 = 0x00080000;
	pub const TEXTURESTAGE:    u32 = 0x00100000;
	pub const FVF:             u32 = 0x00200000;
	pub const SRCVBHANDLE:     u32 = 0x00400000;
	pub const DEPTH:           u32 = 0x00800000;
	pub const ALL:             u32 = 0x00FFF9EE;
}

#[allow(non_snake_case)]
pub mod DDSCAPS {
	pub const RESERVED1:                 u128 = 0x00000001;
	pub const ALPHA:                     u128 = 0x00000002;
	pub const BACKBUFFER:                u128 = 0x00000004;
	pub const COMPLEX:                   u128 = 0x00000008;
	pub const FLIP:                      u128 = 0x00000010;
	pub const FRONTBUFFER:               u128 = 0x00000020;
	pub const OFFSCREENPLAIN:            u128 = 0x00000040;
	pub const OVERLAY:                   u128 = 0x00000080;
	pub const PALETTE:                   u128 = 0x00000100;
	pub const PRIMARYSURFACE:            u128 = 0x00000200;
	pub const PRIMARYSURFACELEFT:        u128 = 0x00000400;
	pub const SYSTEMMEMORY:              u128 = 0x00000800;
	pub const TEXTURE:                   u128 = 0x00001000;
	pub const _3DDEVICE:                 u128 = 0x00002000;
	pub const VIDEOMEMORY:               u128 = 0x00004000;
	pub const VISIBLE:                   u128 = 0x00008000;
	pub const WRITEONLY:                 u128 = 0x00010000;
	pub const ZBUFFER:                   u128 = 0x00020000;
	pub const OWNDC:                     u128 = 0x00040000;
	pub const LIVEVIDEO:                 u128 = 0x00080000;
	pub const HWCODEC:                   u128 = 0x00100000;
	pub const MODEX:                     u128 = 0x00200000;
	pub const MIPMAP:                    u128 = 0x00400000;
	pub const RESERVED2:                 u128 = 0x00800000;
	pub const ALLOCONLOAD:               u128 = 0x04000000;
	pub const VIDEOPORT:                 u128 = 0x08000000;
	pub const LOCALVIDMEM:               u128 = 0x10000000;
	pub const NONLOCALVIDMEM:            u128 = 0x20000000;
	pub const STANDARDVGAMODE:           u128 = 0x40000000;
	pub const OPTIMIZED:                 u128 = 0x80000000;

	pub const HARDWAREDEINTERLACE:       u128 = 0x00000002 << 32;
	pub const HINTDYNAMIC:               u128 = 0x00000004 << 32;
	pub const HINTSTATIC:                u128 = 0x00000008 << 32;
	pub const TEXTUREMANAGE:             u128 = 0x00000010 << 32;
	pub const RESERVED3:                 u128 = 0x00000020 << 32;
	pub const RESERVED4:                 u128 = 0x00000040 << 32;
	pub const OPAQUE:                    u128 = 0x00000080 << 32;
	pub const HINTANTIALIASING:          u128 = 0x00000100 << 32;
	pub const CUBEMAP:                   u128 = 0x00000200 << 32;
	pub const CUBEMAP_POSITIVEX:         u128 = 0x00000400 << 32;
	pub const CUBEMAP_NEGATIVEX:         u128 = 0x00000800 << 32;
	pub const CUBEMAP_POSITIVEY:         u128 = 0x00001000 << 32;
	pub const CUBEMAP_NEGATIVEY:         u128 = 0x00002000 << 32;
	pub const CUBEMAP_POSITIVEZ:         u128 = 0x00004000 << 32;
	pub const CUBEMAP_NEGATIVEZ:         u128 = 0x00008000 << 32;
	pub const CUBEMAP_ALLFACES:          u128 = CUBEMAP_POSITIVEX | CUBEMAP_NEGATIVEX | CUBEMAP_POSITIVEY | CUBEMAP_NEGATIVEY | CUBEMAP_POSITIVEZ | CUBEMAP_NEGATIVEZ;
	pub const MIPMAPSUBLEVEL:            u128 = 0x00010000 << 32;
	pub const D3DTEXTUREMANAGE:          u128 = 0x00020000 << 32;
	pub const DONOTPERSIST:              u128 = 0x00040000 << 32;
	pub const STEREOSURFACELEFT:         u128 = 0x00080000 << 32;
	pub const VOLUME:                    u128 = 0x00200000 << 32;
	pub const NOTUSERLOCKABLE:           u128 = 0x00400000 << 32;
	pub const POINTS:                    u128 = 0x00800000 << 32;
	pub const RTPATCHES:                 u128 = 0x01000000 << 32;
	pub const NPATCHES:                  u128 = 0x02000000 << 32;
	pub const RESERVED5:                 u128 = 0x04000000 << 32;
	pub const DISCARDBACKBUFFER:         u128 = 0x10000000 << 32;
	pub const ENABLEALPHACHANNEL:        u128 = 0x20000000 << 32;
	pub const EXTENDEDFORMATPRIMARY:     u128 = 0x40000000 << 32;
	pub const ADDITIONALPRIMARY:         u128 = 0x80000000 << 32;

	pub const MULTISAMPLE_MASK:          u128 = 0x0000001F << 64;
	pub const MULTISAMPLE_QUALITY_MASK:  u128 = 0x000000E0 << 64;
	pub const MULTISAMPLE_QUALITY_SHIFT: u32 = 5 + 64;
	pub const RESERVED6:                 u128 = 0x00000100 << 64;
	pub const RESERVED7:                 u128 = 0x00000200 << 64;
	pub const LIGHTWEIGHTMIPMAP:         u128 = 0x00000400 << 64;
	pub const AUTOGENMIPMAP:             u128 = 0x00000800 << 64;
	pub const DMAP:                      u128 = 0x00001000 << 64;
	pub const CREATESHAREDRESOURCE:      u128 = 0x00002000 << 64;
	pub const READONLYRESOURCE:          u128 = 0x00004000 << 64;
	pub const OPENSHAREDRESOURCE:        u128 = 0x00008000 << 64;
}

#[allow(non_snake_case)]
pub mod DDPF {
	pub const ALPHAPIXELS:       u32 = 0x00000001;
	pub const ALPHA:             u32 = 0x00000002;
	pub const FOURCC:            u32 = 0x00000004;
	pub const PALETTEINDEXED4:   u32 = 0x00000008;
	pub const PALETTEINDEXEDTO8: u32 = 0x00000010;
	pub const PALETTEINDEXED8:   u32 = 0x00000020;
	pub const RGB:               u32 = 0x00000040;
	pub const COMPRESSED:        u32 = 0x00000080;
	pub const RGBTOYUV:          u32 = 0x00000100;
	pub const YUV:               u32 = 0x00000200;
	pub const ZBUFFER:           u32 = 0x00000400;
	pub const PALETTEINDEXED1:   u32 = 0x00000800;
	pub const PALETTEINDEXED2:   u32 = 0x00001000;
	pub const ZPIXELS:           u32 = 0x00002000;
	pub const STENCILBUFFER:     u32 = 0x00004000;
	pub const ALPHAPREMULT:      u32 = 0x00008000;
	pub const LUMINANCE:         u32 = 0x00020000;
	pub const BUMPLUMINANCE:     u32 = 0x00040000;
	pub const BUMPDUDV:          u32 = 0x00080000;
}

#[allow(non_snake_case)]
pub mod RESOURCE_DIMENSION {
	pub const UNKNOWN: u32 = 0;
	pub const BUFFER: u32 = 1;
	pub const TEXTURE1D: u32 = 2;
	pub const TEXTURE2D: u32 = 3;
	pub const TEXTURE3D: u32 = 4;
}

#[allow(non_snake_case)]
pub mod RESOURCE_MISC {
	pub const GENERATE_MIPS:                   u32 = 0x00000001;
	pub const SHARED:                          u32 = 0x00000002;
	pub const TEXTURECUBE:                     u32 = 0x00000004;
	pub const D3D10_SHARED_KEYEDMUTEX:         u32 = 0x00000010;
	pub const D3D10_GDI_COMPATIBLE:            u32 = 0x00000020;
	pub const DRAWINDIRECT_ARGS:               u32 = 0x00000010;
	pub const BUFFER_ALLOW_RAW_VIEWS:          u32 = 0x00000020;
	pub const BUFFER_STRUCTURED:               u32 = 0x00000040;
	pub const RESOURCE_CLAMP:                  u32 = 0x00000080;
	pub const D3D11_SHARED_KEYEDMUTEX:         u32 = 0x00000100;
	pub const D3D11_GDI_COMPATIBLE:            u32 = 0x00000200;
	pub const SHARED_NTHANDLE:                 u32 = 0x00000800;
	pub const RESTRICTED_CONTENT:              u32 = 0x00001000;
	pub const RESTRICT_SHARED_RESOURCE:        u32 = 0x00002000;
	pub const RESTRICT_SHARED_RESOURCE_DRIVER: u32 = 0x00004000;
	pub const GUARDED:                         u32 = 0x00008000;
	pub const TILE_POOL:                       u32 = 0x00020000;
	pub const TILED:                           u32 = 0x00040000;
	pub const HW_PROTECTED:                    u32 = 0x00080000;
	pub const SHARED_DISPLAYABLE:              u32 = 0x00100000;
	pub const SHARED_EXCLUSIVE_WRITER:         u32 = 0x00200000;
}

#[allow(non_snake_case)]
pub mod ALPHA_MODE {
	pub const UNKNOWN:       u32 = 0x0;
	pub const STRAIGHT:      u32 = 0x1;
	pub const PREMULTIPLIED: u32 = 0x2;
	pub const OPAQUE:        u32 = 0x3;
	pub const CUSTOM:        u32 = 0x4;
	pub const MASK:          u32 = 0x7;
}

#[allow(non_snake_case)]
pub mod DXGI_FORMAT {
	pub const UNKNOWN:                    u32 = 0;
	pub const R32G32B32A32_TYPELESS:      u32 = 1;
	pub const R32G32B32A32_FLOAT:         u32 = 2;
	pub const R32G32B32A32_UINT:          u32 = 3;
	pub const R32G32B32A32_SINT:          u32 = 4;
	pub const R32G32B32_TYPELESS:         u32 = 5;
	pub const R32G32B32_FLOAT:            u32 = 6;
	pub const R32G32B32_UINT:             u32 = 7;
	pub const R32G32B32_SINT:             u32 = 8;
	pub const R16G16B16A16_TYPELESS:      u32 = 9;
	pub const R16G16B16A16_FLOAT:         u32 = 10;
	pub const R16G16B16A16_UNORM:         u32 = 11;
	pub const R16G16B16A16_UINT:          u32 = 12;
	pub const R16G16B16A16_SNORM:         u32 = 13;
	pub const R16G16B16A16_SINT:          u32 = 14;
	pub const R32G32_TYPELESS:            u32 = 15;
	pub const R32G32_FLOAT:               u32 = 16;
	pub const R32G32_UINT:                u32 = 17;
	pub const R32G32_SINT:                u32 = 18;
	pub const R32G8X24_TYPELESS:          u32 = 19;
	pub const D32_FLOAT_S8X24_UINT:       u32 = 20;
	pub const R32_FLOAT_X8X24_TYPELESS:   u32 = 21;
	pub const X32_TYPELESS_G8X24_UINT:    u32 = 22;
	pub const R10G10B10A2_TYPELESS:       u32 = 23;
	pub const R10G10B10A2_UNORM:          u32 = 24;
	pub const R10G10B10A2_UINT:           u32 = 25;
	pub const R11G11B10_FLOAT:            u32 = 26;
	pub const R8G8B8A8_TYPELESS:          u32 = 27;
	pub const R8G8B8A8_UNORM:             u32 = 28;
	pub const R8G8B8A8_UNORM_SRGB:        u32 = 29;
	pub const R8G8B8A8_UINT:              u32 = 30;
	pub const R8G8B8A8_SNORM:             u32 = 31;
	pub const R8G8B8A8_SINT:              u32 = 32;
	pub const R16G16_TYPELESS:            u32 = 33;
	pub const R16G16_FLOAT:               u32 = 34;
	pub const R16G16_UNORM:               u32 = 35;
	pub const R16G16_UINT:                u32 = 36;
	pub const R16G16_SNORM:               u32 = 37;
	pub const R16G16_SINT:                u32 = 38;
	pub const R32_TYPELESS:               u32 = 39;
	pub const D32_FLOAT:                  u32 = 40;
	pub const R32_FLOAT:                  u32 = 41;
	pub const R32_UINT:                   u32 = 42;
	pub const R32_SINT:                   u32 = 43;
	pub const R24G8_TYPELESS:             u32 = 44;
	pub const D24_UNORM_S8_UINT:          u32 = 45;
	pub const R24_UNORM_X8_TYPELESS:      u32 = 46;
	pub const X24_TYPELESS_G8_UINT:       u32 = 47;
	pub const R8G8_TYPELESS:              u32 = 48;
	pub const R8G8_UNORM:                 u32 = 49;
	pub const R8G8_UINT:                  u32 = 50;
	pub const R8G8_SNORM:                 u32 = 51;
	pub const R8G8_SINT:                  u32 = 52;
	pub const R16_TYPELESS:               u32 = 53;
	pub const R16_FLOAT:                  u32 = 54;
	pub const D16_UNORM:                  u32 = 55;
	pub const R16_UNORM:                  u32 = 56;
	pub const R16_UINT:                   u32 = 57;
	pub const R16_SNORM:                  u32 = 58;
	pub const R16_SINT:                   u32 = 59;
	pub const R8_TYPELESS:                u32 = 60;
	pub const R8_UNORM:                   u32 = 61;
	pub const R8_UINT:                    u32 = 62;
	pub const R8_SNORM:                   u32 = 63;
	pub const R8_SINT:                    u32 = 64;
	pub const A8_UNORM:                   u32 = 65;
	pub const R1_UNORM:                   u32 = 66;
	pub const R9G9B9E5_SHAREDEXP:         u32 = 67;
	pub const R8G8_B8G8_UNORM:            u32 = 68;
	pub const G8R8_G8B8_UNORM:            u32 = 69;
	pub const BC1_TYPELESS:               u32 = 70;
	pub const BC1_UNORM:                  u32 = 71;
	pub const BC1_UNORM_SRGB:             u32 = 72;
	pub const BC2_TYPELESS:               u32 = 73;
	pub const BC2_UNORM:                  u32 = 74;
	pub const BC2_UNORM_SRGB:             u32 = 75;
	pub const BC3_TYPELESS:               u32 = 76;
	pub const BC3_UNORM:                  u32 = 77;
	pub const BC3_UNORM_SRGB:             u32 = 78;
	pub const BC4_TYPELESS:               u32 = 79;
	pub const BC4_UNORM:                  u32 = 80;
	pub const BC4_SNORM:                  u32 = 81;
	pub const BC5_TYPELESS:               u32 = 82;
	pub const BC5_UNORM:                  u32 = 83;
	pub const BC5_SNORM:                  u32 = 84;
	pub const B5G6R5_UNORM:               u32 = 85;
	pub const B5G5R5A1_UNORM:             u32 = 86;
	pub const B8G8R8A8_UNORM:             u32 = 87;
	pub const B8G8R8X8_UNORM:             u32 = 88;
	pub const R10G10B10_XR_BIAS_A2_UNORM: u32 = 89;
	pub const B8G8R8A8_TYPELESS:          u32 = 90;
	pub const B8G8R8A8_UNORM_SRGB:        u32 = 91;
	pub const B8G8R8X8_TYPELESS:          u32 = 92;
	pub const B8G8R8X8_UNORM_SRGB:        u32 = 93;
	pub const BC6H_TYPELESS:              u32 = 94;
	pub const BC6H_UF16:                  u32 = 95;
	pub const BC6H_SF16:                  u32 = 96;
	pub const BC7_TYPELESS:               u32 = 97;
	pub const BC7_UNORM:                  u32 = 98;
	pub const BC7_UNORM_SRGB:             u32 = 99;
	pub const AYUV:                       u32 = 100;
	pub const Y410:                       u32 = 101;
	pub const Y416:                       u32 = 102;
	pub const NV12:                       u32 = 103;
	pub const P010:                       u32 = 104;
	pub const P016:                       u32 = 105;
	pub const _420_OPAQUE:                u32 = 106;
	pub const YUY2:                       u32 = 107;
	pub const Y210:                       u32 = 108;
	pub const Y216:                       u32 = 109;
	pub const NV11:                       u32 = 110;
	pub const AI44:                       u32 = 111;
	pub const IA44:                       u32 = 112;
	pub const P8:                         u32 = 113;
	pub const A8P8:                       u32 = 114;
	pub const B4G4R4A4_UNORM:             u32 = 115;
	pub const P208:                       u32 = 130;
	pub const V208:                       u32 = 131;
	pub const V408:                       u32 = 132;
}
