import fs from "node:fs";
import assert from "node:assert/strict";
const css=fs.readFileSync("launcher/gui/src/ui/tokens.css","utf8");
const java=fs.readFileSync("aether-client/src/main/java/dev/aether/client/design/AetherDesignSystem.java","utf8");
const config=fs.readFileSync("aether-client/src/main/java/dev/aether/client/config/AetherConfig.java","utf8");
const gui=fs.readFileSync("aether-client/src/main/java/dev/aether/client/gui/ConfigScreen.java","utf8");
const channels=["bg","bg-elevated","surface","surface-hover","surface-active","border","border-strong","accent","text-primary","text-secondary","text-muted","text-dim"];
const palettes=["graphite","oxide","field","blueprint"];
function cssColor(name){
 const match=css.match(new RegExp(`--palette-${name}:\\s*(#[0-9a-fA-F]{6})\\s*;`));
 assert.ok(match,`Missing CSS token --palette-${name}`);
 return match[1].toUpperCase();
}
function javaValues(id){
 const start=java.indexOf(`${id.toUpperCase()}("${id}"`);
 assert.notEqual(start,-1,`Missing Java palette ${id}`);
 const commaEnd=java.indexOf(")),",start);
 const semicolonEnd=java.indexOf("));",start);
 const candidates=[commaEnd,semicolonEnd].filter(value=>value!==-1);
 const end=candidates.length?Math.min(...candidates):-1;
 assert.notEqual(end,-1,`Palette ${id} is not closed`);
 return [...java.slice(start,end).matchAll(/0xFF([0-9A-Fa-f]{6})/g)].map(m=>`#${m[1]}`.toUpperCase());
}
for(const palette of palettes){
 const expected=channels.map(channel=>cssColor(`${palette}-${channel}`));
 const actual=javaValues(palette);
 assert.equal(actual.length,12,`${palette}: expected 12 Java channels, found ${actual.length}`);
 assert.deepEqual(actual,expected,`${palette}: launcher and in-game values differ`);
}
for(const value of ["contour","draft","quiet"])assert.ok(java.includes(`"${value}"`),`Missing treatment ${value}`);
for(const field of ["colorway","surfaceTreatment","customAccent"])
 assert.ok(config.includes(`String ${field}`)||config.includes(`boolean ${field}`),`Missing preference ${field}`);
assert.ok(gui.includes("SurfaceTreatment"),"ConfigScreen must expose surface treatments");
console.log("Aether Design System: 48 launcher/client palette channels match.");
