package dev.aether.client;

import dev.aether.client.config.AetherConfig;
import dev.aether.client.design.AetherDesignSystem;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class AetherDesignSystemTest {
	@Test
	@DisplayName("four materials expose their signature accents")
	void materialSignatureAccents(){
		assertEquals(0xFFE7E9EC,AetherDesignSystem.Colorway.GRAPHITE.palette().accent());
		assertEquals(0xFFD99A69,AetherDesignSystem.Colorway.OXIDE.palette().accent());
		assertEquals(0xFFC0D18A,AetherDesignSystem.Colorway.FIELD.palette().accent());
		assertEquals(0xFF83C5DF,AetherDesignSystem.Colorway.BLUEPRINT.palette().accent());
	}
	@Test
	@DisplayName("invalid identifiers fall back safely")
	void identifiersHaveSafeDefaults(){
		assertEquals(AetherDesignSystem.Colorway.OXIDE,AetherDesignSystem.Colorway.fromId(" OxIdE "));
		assertEquals(AetherDesignSystem.Colorway.GRAPHITE,AetherDesignSystem.Colorway.fromId("unknown"));
		assertEquals(AetherDesignSystem.SurfaceTreatment.DRAFT,AetherDesignSystem.SurfaceTreatment.fromId("DRAFT"));
		assertEquals(AetherDesignSystem.SurfaceTreatment.CONTOUR,AetherDesignSystem.SurfaceTreatment.fromId(null));
	}
	@Test
	@DisplayName("signature and personal accents have readable foreground contrast")
	void accentForegroundContrast(){
		for(var colorway:AetherDesignSystem.Colorway.values())
			assertTrue(colorway.palette().contrastRatioWithOnAccent()>=4.5,colorway.id());
		AetherConfig config=new AetherConfig();config.customAccent=true;config.accentColor="#17304A";
		assertTrue(AetherDesignSystem.resolve(config).contrastRatioWithOnAccent()>=4.5);
	}
	@Test
	@DisplayName("material accent is inherited unless a personal override is enabled")
	void signatureAccentFollowsMaterial(){
		AetherConfig config=new AetherConfig();config.colorway="blueprint";
		assertEquals(0xFF83C5DF,AetherDesignSystem.resolve(config).accent());
		config.customAccent=true;config.accentColor="#12AB34";
		assertEquals(0xFF12AB34,AetherDesignSystem.resolve(config).accent());
	}
}
