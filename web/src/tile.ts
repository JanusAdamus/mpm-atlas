// Binary tile v1 decoder ("ATT1"; core/atlas-server/src/tiles.rs is the reference).
// Little-endian, sections 4-byte aligned, so every numeric column is a typed-array
// view over the response buffer: no JSON parse, no per-node objects.

export const TILE_MAGIC = 0x31545441; // "ATT1" read as a little-endian u32
export const NONE = 0xffffffff;
export const KIND_ENTITY = 0;
export const KIND_CLUSTER = 1;
export const NO_TYPE = 255;

export interface Tile {
  z: number;
  x: number;
  y: number;
  maxZoom: number;
  bbox: Float32Array; // [x0, y0, x1, y1]
  layer: number; // Projection code
  layoutVersion: string;
  strings: string[];
  /** Items = tile nodes [0, nNodes) followed by external edge endpoints. */
  nNodes: number;
  nItems: number;
  px: Float32Array;
  py: Float32Array;
  size: Uint32Array;
  id: Uint32Array; // string index
  label: Uint32Array; // string index
  community: Uint32Array; // string index
  priority: Uint32Array; // NONE for clusters
  centralOff: Uint32Array;
  central: Uint32Array; // string indexes
  kind: Uint8Array;
  type: Uint8Array; // EntityType code, NO_TYPE for clusters
  nEdges: number;
  edgeSrc: Uint32Array; // item index
  edgeDst: Uint32Array;
  edgeCount: Uint32Array;
  edgeRel: Uint8Array;
  edgeState: Uint8Array;
  nAggs: number;
  aggItem: Uint32Array;
  aggMembers: Uint32Array;
  aggHidden: Uint32Array;
  aggRelOff: Uint32Array;
  aggRelCount: Uint32Array;
  aggRel: Uint8Array;
  /** Tile edges over the per-tile budget, counted per relation (never dropped silently). */
  hiddenCount: Uint32Array;
  hiddenRel: Uint8Array;
}

const HEADER_BYTES = 76;

class Cursor {
  off = 0;
  private readonly buf: ArrayBuffer;
  constructor(buf: ArrayBuffer) {
    this.buf = buf;
  }
  private need(bytes: number): void {
    if (this.off + bytes > this.buf.byteLength) throw new Error("tile truncated");
  }
  u32(n: number): Uint32Array {
    this.need(n * 4);
    const a = new Uint32Array(this.buf, this.off, n);
    this.off += n * 4;
    return a;
  }
  f32(n: number): Float32Array {
    this.need(n * 4);
    const a = new Float32Array(this.buf, this.off, n);
    this.off += n * 4;
    return a;
  }
  u8(n: number): Uint8Array {
    this.need(n);
    const a = new Uint8Array(this.buf, this.off, n);
    this.off += n;
    return a;
  }
  pad(): void {
    this.off = (this.off + 3) & ~3;
  }
}

export function decodeTile(buf: ArrayBuffer): Tile {
  if (buf.byteLength < HEADER_BYTES) throw new Error(`tile too short (${buf.byteLength} bytes)`);
  const dv = new DataView(buf);
  const word = (o: number): number => dv.getUint32(o, true);
  if (word(0) !== TILE_MAGIC) throw new Error("not an ATT1 tile");
  const c = new Cursor(buf);
  c.off = 20;
  const bbox = c.f32(4);
  const layer = word(36);
  const versionIx = word(40);
  c.off = 44;
  const [nStrings, nNodes, nExternal, nEdges, nAggs, nAggRel, nHidden, nCentral] = c.u32(8);
  const strOff = c.u32(nStrings + 1);
  const bytes = c.u8(strOff[nStrings]);
  c.pad();
  const dec = new TextDecoder();
  const strings = new Array<string>(nStrings);
  for (let i = 0; i < nStrings; i++) strings[i] = dec.decode(bytes.subarray(strOff[i], strOff[i + 1]));

  const n = nNodes + nExternal;
  const px = c.f32(n);
  const py = c.f32(n);
  const size = c.u32(n);
  const id = c.u32(n);
  const label = c.u32(n);
  const community = c.u32(n);
  const priority = c.u32(n);
  const centralOff = c.u32(n + 1);
  const central = c.u32(nCentral);
  const kind = c.u8(n);
  const type = c.u8(n);
  c.pad();
  const edgeSrc = c.u32(nEdges);
  const edgeDst = c.u32(nEdges);
  const edgeCount = c.u32(nEdges);
  const edgeRel = c.u8(nEdges);
  const edgeState = c.u8(nEdges);
  c.pad();
  const aggItem = c.u32(nAggs);
  const aggMembers = c.u32(nAggs);
  const aggHidden = c.u32(nAggs);
  const aggRelOff = c.u32(nAggs + 1);
  const aggRelCount = c.u32(nAggRel);
  const aggRel = c.u8(nAggRel);
  c.pad();
  const hiddenCount = c.u32(nHidden);
  const hiddenRel = c.u8(nHidden);
  c.pad();
  if (c.off !== buf.byteLength) throw new Error(`tile has ${buf.byteLength - c.off} trailing bytes`);
  for (let i = 0; i < nEdges; i++) {
    if (edgeSrc[i] >= n || edgeDst[i] >= n) throw new Error("edge endpoint out of range");
  }
  return {
    z: word(4), x: word(8), y: word(12), maxZoom: word(16), bbox, layer,
    layoutVersion: strings[versionIx] ?? "", strings,
    nNodes, nItems: n, px, py, size, id, label, community, priority, centralOff, central, kind, type,
    nEdges, edgeSrc, edgeDst, edgeCount, edgeRel, edgeState,
    nAggs, aggItem, aggMembers, aggHidden, aggRelOff, aggRelCount, aggRel,
    hiddenCount, hiddenRel,
  };
}

/** Ids of item `i`'s most central members (clusters only). */
export function centralOf(t: Tile, i: number): string[] {
  const out: string[] = [];
  for (let k = t.centralOff[i]; k < t.centralOff[i + 1]; k++) out.push(t.strings[t.central[k]]);
  return out;
}
