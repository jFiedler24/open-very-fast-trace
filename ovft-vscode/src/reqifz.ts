import * as zlib from "zlib";

const EOCD_SIG = 0x06054b50;
const CDIR_SIG = 0x02014b50;
const LOCAL_SIG = 0x04034b50;

interface ZipEntry {
  name: string;
  method: number;
  compressedSize: number;
  localOffset: number;
}

/**
 * Extract the first `.reqif` document from a `.reqifz` ZIP archive.
 * Supports stored and deflated entries, which covers ReqIFz files in
 * the wild without pulling in a zip dependency.
 * [impl->req~reqif-import~1]
 */
export function extractReqifXml(data: Buffer): string {
  const min = Math.max(0, data.length - 22 - 0xffff);
  let eocd = -1;
  for (let i = data.length - 22; i >= min; i--) {
    if (data.readUInt32LE(i) === EOCD_SIG) {
      eocd = i;
      break;
    }
  }
  if (eocd < 0) {
    throw new Error("Not a ZIP archive (end of central directory not found)");
  }

  const count = data.readUInt16LE(eocd + 10);
  let offset = data.readUInt32LE(eocd + 16);
  const entries: ZipEntry[] = [];
  for (let i = 0; i < count; i++) {
    if (data.readUInt32LE(offset) !== CDIR_SIG) {
      throw new Error("Corrupt ZIP central directory");
    }
    const nameLen = data.readUInt16LE(offset + 28);
    const extraLen = data.readUInt16LE(offset + 30);
    const commentLen = data.readUInt16LE(offset + 32);
    entries.push({
      method: data.readUInt16LE(offset + 10),
      compressedSize: data.readUInt32LE(offset + 20),
      localOffset: data.readUInt32LE(offset + 42),
      name: data.toString("utf-8", offset + 46, offset + 46 + nameLen),
    });
    offset += 46 + nameLen + extraLen + commentLen;
  }

  const entry = entries.find(
    (e) => e.name.toLowerCase().endsWith(".reqif") && !e.name.endsWith("/")
  );
  if (!entry) {
    throw new Error("ReqIFz archive contains no .reqif document");
  }

  const lo = entry.localOffset;
  if (data.readUInt32LE(lo) !== LOCAL_SIG) {
    throw new Error("Corrupt ZIP local file header");
  }
  const start = lo + 30 + data.readUInt16LE(lo + 26) + data.readUInt16LE(lo + 28);
  const raw = data.subarray(start, start + entry.compressedSize);

  if (entry.method === 0) {
    return raw.toString("utf-8");
  }
  if (entry.method === 8) {
    return zlib.inflateRawSync(raw).toString("utf-8");
  }
  throw new Error(
    `Unsupported ZIP compression method ${entry.method} (only stored and deflate are supported)`
  );
}
