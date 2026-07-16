/* Art catalog client — identity → presentation (FS-HDG) */

let artCatalog = null;

async function ensureArtCatalog(versionHint) {
  if (
    artCatalog &&
    (versionHint == null || artCatalog.catalog_version === versionHint)
  ) {
    return artCatalog;
  }
  const r = await fetch("/api/art");
  const d = await r.json();
  if (!d.ok) throw new Error("art catalog failed");
  artCatalog = d;
  return artCatalog;
}

function decodeFeatIds(payload) {
  if (!payload || payload.enc !== "u16le_b64" || !payload.data) {
    return null;
  }
  const bin = atob(payload.data);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  const n = bytes.length >> 1;
  const out = new Uint16Array(n);
  for (let i = 0; i < n; i++) {
    out[i] = bytes[i * 2] | (bytes[i * 2 + 1] << 8);
  }
  return out;
}

function featKey(id) {
  return String(id);
}

function lookForFeat(id) {
  const c = artCatalog;
  if (!c || !c.feats) {
    return { glyph: "?", material: "void", layer: 0, name: "?" };
  }
  const f = c.feats[featKey(id)];
  if (!f) {
    return { glyph: "?", material: "basalt", layer: 0, name: "#" + id };
  }
  return f;
}

function lookForEntity(ent) {
  const c = artCatalog;
  if (!c) {
    return { glyph: ent.glyph || "?", material: "ui_info" };
  }
  if (ent.kind === "monster" && ent.race_id != null && c.races) {
    const r = c.races[featKey(ent.race_id)];
    if (r) return r;
  }
  if (ent.kind === "item" && ent.kind_id != null && c.objects) {
    const o = c.objects[featKey(ent.kind_id)];
    if (o) return o;
  }
  const d = c.entity_defaults && c.entity_defaults[ent.kind];
  if (d) return d;
  return { glyph: ent.glyph || "?", material: "ui_info" };
}

function materialColor(material, theme) {
  if (theme && theme.materials && theme.materials[material]) {
    return theme.materials[material];
  }
  if (artCatalog && artCatalog.materials && artCatalog.materials[material]) {
    return artCatalog.materials[material];
  }
  return "#8b93a0";
}
