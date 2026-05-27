import { parseJsonMap, stringValue } from './api';

export function jsonEntries(value: string): Array<[string, string]> {
  const entries = parseJsonMap(value);
  if (entries.length) {
    return entries;
  }
  if (!value.trim()) {
    return [];
  }
  try {
    const parsed = JSON.parse(value) as unknown;
    if (Array.isArray(parsed)) {
      return parsed.map((item, index) => [String(index + 1), stringValue(item) ?? '']);
    }
    return [['value', stringValue(parsed) ?? value]];
  } catch {
    return [['value', value]];
  }
}
