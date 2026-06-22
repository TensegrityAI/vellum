/**
 * Minimal single-range diff between two strings, in **UTF-16 code units** (the
 * space the DOM, `<textarea>`, and `EditContext` all report). This is how the view
 * turns a device's new full value into one `delete`+`insert` against the core
 * instead of clearing and reinserting the whole buffer each keystroke (Increment-1
 * blocker #1). The caller converts the returned UTF-16 offsets to UTF-8 byte
 * offsets via the wasm `utf16_to_byte` helper before touching the byte-indexed core.
 */
export interface Diff {
  /** Where the change starts, in UTF-16 code units (length of the common prefix). */
  readonly utf16Start: number;
  /** How many UTF-16 code units of the old value were removed at `utf16Start`. */
  readonly utf16RemovedLen: number;
  /** The text inserted at `utf16Start` (UTF-16). */
  readonly inserted: string;
}

const isHighSurrogate = (code: number): boolean => code >= 0xd800 && code <= 0xdbff;
const isLowSurrogate = (code: number): boolean => code >= 0xdc00 && code <= 0xdfff;

/**
 * Compute the single changed range between `oldValue` and `newValue` by stripping
 * the common prefix and the common suffix (in UTF-16 code units). Surrogate-safe:
 * a prefix never ends, and a suffix never starts, in the middle of a surrogate
 * pair — splitting one would yield an offset the core would reject. For a typical
 * keystroke the result touches only the edited run.
 */
export function computeDiff(oldValue: string, newValue: string): Diff {
  const oldLen = oldValue.length;
  const newLen = newValue.length;
  const maxShared = Math.min(oldLen, newLen);

  // Longest common prefix, code unit by code unit.
  let prefix = 0;
  while (prefix < maxShared && oldValue.charCodeAt(prefix) === newValue.charCodeAt(prefix)) {
    prefix++;
  }
  // Never end the prefix between a high surrogate and its low surrogate: if the
  // last shared unit is a high surrogate, the low half differs (or is absent), so
  // step back to keep the pair whole.
  if (prefix > 0 && isHighSurrogate(oldValue.charCodeAt(prefix - 1))) {
    prefix--;
  }

  // Longest common suffix, not overlapping the prefix we already claimed.
  let suffix = 0;
  const maxSuffix = maxShared - prefix;
  while (
    suffix < maxSuffix &&
    oldValue.charCodeAt(oldLen - 1 - suffix) === newValue.charCodeAt(newLen - 1 - suffix)
  ) {
    suffix++;
  }
  // Never start the suffix between a high surrogate and its low surrogate: if the
  // first shared unit is a low surrogate, its high half differs, so step back.
  if (suffix > 0 && isLowSurrogate(oldValue.charCodeAt(oldLen - suffix))) {
    suffix--;
  }

  return {
    utf16Start: prefix,
    utf16RemovedLen: oldLen - prefix - suffix,
    inserted: newValue.slice(prefix, newLen - suffix),
  };
}
