package dev.aether.client.design;

import dev.aether.client.config.AetherConfig;
import dev.aether.client.util.ColorUtil;

/** In-game semantic tokens mirrored by launcher/gui/src/ui/tokens.css. */
public final class AetherDesignSystem {
	private AetherDesignSystem() {}

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
}
