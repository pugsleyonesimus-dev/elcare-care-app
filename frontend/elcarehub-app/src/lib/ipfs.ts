// ─────────────────────────────────────────────────────────────
// lib/ipfs.ts — IPFS upload helpers via Pinata REST API
// ─────────────────────────────────────────────────────────────
//
// Artwork metadata schema (stored on IPFS):
// {
//   "title": "…",
//   "description": "…",
//   "artist": "…",
//   "image": "ipfs://CID",
//   "year": "2024"
// }
// ─────────────────────────────────────────────────────────────

import axios from "axios";
import { config } from "./config";

/** Artwork metadata stored on IPFS */
export interface ArtworkMetadata {
  title: string;
  description: string;
  artist: string;
  /** Must be in the form "ipfs://CID" */
  image: string;
  year: string;
  category: string;
}

/** Result of any IPFS upload */
export interface IpfsUploadResult {
  cid: string;
  url: string;
}

// ── Upload a File (image) ─────────────────────────────────────

/**
 * Uploads an artwork image to IPFS via Pinata.
 * Returns the raw CID string.
 */
export async function uploadImageToIPFS(
  file: File,
  name?: string
): Promise<IpfsUploadResult> {
  const formData = new FormData();
  formData.append("file", file);
  formData.append("name", name ?? file.name);

  const res = await axios.post("/api/ipfs/upload-image", formData, {
    maxBodyLength: Infinity,
  });

  const cid: string = res.data.cid;
  return {
    cid,
    url: `${config.pinataGateway}/ipfs/${cid}`,
  };
}

// ── Upload JSON metadata ──────────────────────────────────────

/**
 * Uploads artwork metadata JSON to IPFS via Pinata.
 * Returns the CID of the metadata file.
 */
export async function uploadMetadataToIPFS(
  metadata: ArtworkMetadata,
  name?: string
): Promise<IpfsUploadResult> {
  const res = await axios.post("/api/ipfs/upload-metadata", {
    metadata,
    name: name ?? `${metadata.title}-metadata.json`,
  });

  const cid: string = res.data.cid;
  return {
    cid,
    url: `${config.pinataGateway}/ipfs/${cid}`,
  };
}

// ── Public fallback IPFS gateways ──────────────────────────────

export const DEFAULT_FALLBACK_GATEWAYS = [
  "https://ipfs.io",
  "https://cloudflare-ipfs.com",
  "https://dweb.link",
];

/** Normalizes an IPFS URI to a clean CID. Strips `ipfs://` prefix. Passes full HTTP URLs through unchanged. */
export function normalizeIpfsUri(uri: string): string {
  if (uri.startsWith("http")) return uri;
  return uri.replace("ipfs://", "").trim();
}

/**
 * Returns an ordered list of gateway URLs for a given CID.
 * The configured primary gateway comes first, followed by public fallbacks.
 * Deduplicates gateways so the same URL is never tried twice.
 */
export function getGatewayUrls(
  cid: string,
  primaryGateway?: string
): string[] {
  const clean = normalizeIpfsUri(cid);
  if (clean.startsWith("http")) return [clean];

  const primary = primaryGateway ?? config.pinataGateway;
  const seen = new Set<string>();
  return [primary, ...DEFAULT_FALLBACK_GATEWAYS].filter((gw) => {
    if (seen.has(gw)) return false;
    seen.add(gw);
    return true;
  }).map((gw) => `${gw.replace(/\/$/, "")}/ipfs/${clean}`);
}

// ── Fetch metadata ────────────────────────────────────────────

/**
 * Fetches and parses artwork metadata JSON from IPFS.
 * `cid` can be a raw CID string or an "ipfs://CID" URI.
 */
export async function fetchMetadata(cid?: string): Promise<ArtworkMetadata> {
  if (!cid) {
    return { title: "Unknown Artwork", description: "", artist: "Unknown", image: "", year: "", category: "" };
  }
  const cleanCid = normalizeIpfsUri(cid);
  const urls = getGatewayUrls(cleanCid);
  let lastError: unknown;
  for (const url of urls) {
    try {
      const res = await axios.get<ArtworkMetadata>(url);
      return res.data;
    } catch (err) {
      lastError = err;
    }
  }
  throw lastError;
}

// ── Utility ───────────────────────────────────────────────────

/** Converts a raw CID string or an IPFS URI to an IPFS gateway URL for image display. Handles full URLs gracefully. */
export function cidToGatewayUrl(cid: string): string {
  return getGatewayUrls(cid)[0];
}
