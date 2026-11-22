package dev.aether.client.design;

import dev.aether.client.config.AetherConfig;
import dev.aether.client.util.ColorUtil;

/** In-game semantic tokens mirrored by launcher/gui/src/ui/tokens.css. */
public final class AetherDesignSystem {
	private AetherDesignSystem() {}

	/** Geometry, rhythm and motion shared by every in-game surface. */
	public static final class Metrics {
		public static final int RADIUS_XS = 3;
		public static final int RADIUS_SM = 4;
		public static final int RADIUS_MD = 6;
		public static final int RADIUS_LG = 8;
		public static final int RADIUS_XL = 12;
		public static final int SPACE_XS = 4;
		public static final int SPACE_SM = 8;
		public static final int SPACE_LG = 16;
		public static final int ENTER_LIFT = 9;
		public static final long ENTER_NANOS = 240_000_000L;
		private Metrics() {}
	}

	/** Colours of the Aurora Nightfall scene and of semantic states, not of a colorway. */
	public static final class Scene {
		public static final int AURORA_GREEN = 0xFF4FE3A1;
		public static final int AURORA_TEAL = 0xFF3EC9D6;
		public static final int AURORA_VIOLET = 0xFF9A8CFF;
		public static final int MOON = 0xFFF4E7C3;
		public static final int STAR = 0xFFEAF3FF;
		public static final int MOUNTAIN = 0xFF06080B;
		public static final int ONLINE = 0xFF46C98B;
		public static final int OFFLINE = 0xFFD9A441;
		private Scene() {}
	}

	/** Accent swatches offered by the configuration, kept out of the screens. */
	public static final int[] ACCENT_PRESETS = {
			0xFFE7E9EC, 0xFFB9C0C8, 0xFF8C949C, 0xFF5E666E, 0xFF7E9C84, 0xFFC0A870,
			0xFFC4897F, 0xFF7E93A6, 0xFF938CB0, 0xFFB08A72, 0xFF7FA89A, 0xFFBFA57C
	};

	/** Colours cycled by a ColorSetting. They are data, not chrome. */
	public static final int[] COLOR_PRESETS = {
			0xFF6C5CE7, 0xFF2ECC8F, 0xFFF0B429, 0xFFE05260,
			0xFF3498DB, 0xFFE67E22, 0xFFFF6CE8, 0xFF00D2D3
	};

	public enum Colorway {
		GRAPHITE("graphite", "Graphite", new Palette(
				0xFF0A0B0D, 0xFF0D0F12, 0xFF121418, 0xFF171A1F, 0xFF1C2026,
				0xFF21252B, 0xFF2C323A, 0xFFE7E9EC, 0xFFF2F4F6, 0xFFA9B1B9,
				0xFF7D868F, 0xFF767F88)),
		OXIDE("oxide", "Oxide", new Palette(
				0xFF100E0C, 0xFF17120F, 0xFF1D1713, 0xFF272019, 0xFF30251D,
				0xFF392D25, 0xFF514033, 0xFFD99A69, 0xFFF5EEE8, 0xFFC4B2A4,
				0xFF9B897A, 0xFF887767)),
		FIELD("field", "Field", new Palette(
				0xFF0C110E, 0xFF111913, 0xFF172019, 0xFF202B22, 0xFF29392C,
				0xFF2B3A2E, 0xFF405544, 0xFFC0D18A, 0xFFEDF3E9, 0xFFAFC0AE,
				0xFF809281, 0xFF718574)),
		BLUEPRINT("blueprint", "Blueprint", new Palette(
				0xFF0A1016, 0xFF0E1821, 0xFF121F2A, 0xFF192A37, 0xFF203747,
				0xFF263B4B, 0xFF35566B, 0xFF83C5DF, 0xFFEDF5F8, 0xFFADBFCA,
				0xFF8198A7, 0xFF708A9B));

		private final String id, label;
		private final Palette palette;
		Colorway(String id,String label,Palette palette){this.id=id;this.label=label;this.palette=palette;}
		public String id(){return id;}
		public String label(){return label;}
		/** A label that fits the four cards of the compact configuration screen. */
		public String compactLabel(){return switch(this){
			case GRAPHITE -> "Graph";
			case OXIDE -> "Oxide";
			case FIELD -> "Field";
			case BLUEPRINT -> "Blue";
		};}
		public Palette palette(){return palette;}
		public static Colorway fromId(String id){
			if(id!=null)for(Colorway value:values())if(value.id.equalsIgnoreCase(id.trim()))return value;
			return GRAPHITE;
		}
	}

	public enum SurfaceTreatment {
		CONTOUR("contour","Contour"), DRAFT("draft","Draft"), QUIET("quiet","Quiet");
		private final String id,label;
		SurfaceTreatment(String id,String label){this.id=id;this.label=label;}
		public String id(){return id;}
		public String label(){return label;}
		public static SurfaceTreatment fromId(String id){
			if(id!=null)for(SurfaceTreatment value:values())if(value.id.equalsIgnoreCase(id.trim()))return value;
			return CONTOUR;
		}
	}

	/** Immutable surface, border, accent and typography tokens. */
	public record Palette(int background,int elevatedBackground,int surface,int surfaceHover,int surfaceActive,
			int border,int borderStrong,int accent,int textPrimary,int textSecondary,int textMuted,int textDim){
		public Palette withAccent(int value){
			return new Palette(background,elevatedBackground,surface,surfaceHover,surfaceActive,border,borderStrong,
					value|0xFF000000,textPrimary,textSecondary,textMuted,textDim);
		}
		/** Selects a black or white foreground according to WCAG relative luminance. */
		public int onAccentText(){return luminance(accent)>0.179?0xFF0A0B0D:0xFFF5F7F9;}
		public double contrastRatioWithOnAccent(){
			double a=luminance(onAccentText()),b=luminance(accent);
			return (Math.max(a,b)+0.05)/(Math.min(a,b)+0.05);
		}
		private static double luminance(int c){
			return 0.2126*linear((c>>>16)&255)+0.7152*linear((c>>>8)&255)+0.0722*linear(c&255);
		}
		private static double linear(int c){
			double v=c/255.0;return v<=0.04045?v/12.92:Math.pow((v+0.055)/1.055,2.4);
		}
	}

	public static Palette resolve(AetherConfig config){
		if(config==null)return Colorway.GRAPHITE.palette();
		Palette palette=Colorway.fromId(config.colorway).palette();
		return config.customAccent?palette.withAccent(ColorUtil.fromHex(config.accentColor,palette.accent())):palette;
	}
	public static SurfaceTreatment surfaceTreatment(AetherConfig config){
		return config==null?SurfaceTreatment.CONTOUR:SurfaceTreatment.fromId(config.surfaceTreatment);
	}

	/** Semantic text roles keep typography out of the screens. */
	public enum TextRole { TITLE, LABEL, HINT, ACCENT }

	public static int text(Palette palette, TextRole role) {
		return switch (role) {
			case TITLE -> palette.textPrimary();
			case LABEL -> palette.textSecondary();
			case HINT -> palette.textDim();
			case ACCENT -> palette.accent();
		};
	}

	/** The surface of a control at rest, hovered or selected. */
	public enum SurfaceState { REST, HOVER, ACTIVE }

	public static int surface(Palette palette, SurfaceState state) {
		return switch (state) {
			case REST -> palette.surface();
			case HOVER -> palette.surfaceHover();
			case ACTIVE -> palette.surfaceActive();
		};
	}

	/** Opaque glass tint: the dark backdrop is already painted behind it. */
	public static int glass(Palette palette) {
		return ColorUtil.lerp(palette.elevatedBackground(), palette.surfaceActive(), 0.62f);
	}

	public static int glassDeep(Palette palette) {
		return ColorUtil.lerp(palette.background(), palette.surface(), 0.58f);
	}

	public static int glassBorder(Palette palette) {
		return ColorUtil.lerp(palette.border(), palette.borderStrong(), 0.65f);
	}

	public static int glassBorder(Palette palette, int accent) {
		return ColorUtil.lerp(glassBorder(palette), accent, 0.28f);
	}

	public static int glassHighlight(Palette palette) {
		return ColorUtil.withAlpha(palette.textPrimary(), 0.10f);
	}

	public static int backdropPattern(Palette palette) {
		return ColorUtil.withAlpha(palette.borderStrong(), 0.14f);
	}

	public static int field(Palette palette, boolean focused) {
		return focused
				? ColorUtil.lerp(palette.background(), palette.surfaceActive(), 0.30f)
				: glassDeep(palette);
	}

	public static int fieldBorder(Palette palette, int accent, boolean focused, boolean hovered) {
		if (focused) return accent;
		return hovered ? ColorUtil.withAlpha(palette.textPrimary(), 0.22f) : glassBorder(palette);
	}

	public static int row(Palette palette, int accent, boolean active, boolean hovered) {
		if (active) return ColorUtil.lerp(palette.surface(), accent | 0xFF000000, 0.14f);
		return hovered ? palette.surfaceHover() : palette.surface();
	}

	public static int rowBorder(Palette palette, int accent, boolean active, boolean hovered) {
		if (active) return ColorUtil.withAlpha(accent, 0.40f);
		return hovered ? ColorUtil.withAlpha(palette.textPrimary(), 0.20f)
				: ColorUtil.withAlpha(palette.textPrimary(), 0.08f);
	}

	public static int switchTrack(Palette palette, int accent, float progress) {
		return ColorUtil.lerp(ColorUtil.withAlpha(palette.textPrimary(), 0.20f),
				ColorUtil.withAlpha(accent, 0.95f), progress);
	}

	public static int switchKnob(Palette palette, float progress) {
		return ColorUtil.lerp(palette.textPrimary(), palette.onAccentText(), progress);
	}

	public static int chip(Palette palette, int accent, boolean armed) {
		return armed ? ColorUtil.lerp(palette.surfaceActive(), accent, 0.35f)
				: ColorUtil.lerp(palette.surface(), palette.elevatedBackground(), 0.55f);
	}

	public static int chipBorder(Palette palette, int accent, boolean armed) {
		return armed ? accent : palette.borderStrong();
	}

	public static int badge(Palette palette, int accent, boolean active, boolean hovered) {
		if (active) return ColorUtil.withAlpha(accent, 0.20f);
		return hovered ? ColorUtil.withAlpha(palette.textPrimary(), 0.10f)
				: ColorUtil.withAlpha(palette.textPrimary(), 0.04f);
	}

	public static int badgeText(Palette palette, int accent, boolean active, boolean hovered) {
		if (active) return accent;
		return hovered ? palette.textPrimary() : palette.textSecondary();
	}

	public static int button(Palette palette, int accent, boolean primary, boolean hovered) {
		if (primary) return ColorUtil.lerp(accent, palette.textPrimary(), hovered ? 0.16f : 0.0f);
		return hovered ? palette.surfaceHover() : palette.surface();
	}

	public static int buttonBorder(Palette palette, int accent, boolean primary) {
		return primary ? accent : palette.borderStrong();
	}

	public static int buttonText(Palette palette, boolean primary) {
		return primary ? palette.onAccentText() : palette.textSecondary();
	}

	public static int sliderTrack(Palette palette) {
		return ColorUtil.withAlpha(palette.textPrimary(), 0.14f);
	}

	public static int sliderFill(int accent) {
		return accent;
	}

	public static int scrollTrack(Palette palette) {
		return ColorUtil.withAlpha(palette.textPrimary(), 0.08f);
	}

	public static int scrollThumb(int accent) {
		return ColorUtil.withAlpha(accent, 0.78f);
	}

	public static int guide(int accent) {
		return ColorUtil.withAlpha(accent, 0.72f);
	}

	public static int glow(int accent, float alpha) {
		return ColorUtil.withAlpha(accent, alpha);
	}
}
