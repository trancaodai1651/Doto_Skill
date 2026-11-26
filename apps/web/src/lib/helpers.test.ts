import { describe, expect, it } from 'vitest';
import { formatBytes, formatDuration, formatEta, formatResolution, guessDeviceName, secondsToMs } from './format';
import { readDesktopToken } from './desktop';
import { upsertById } from './list';
import { translate } from '../i18n/core';
import { vi as viDict } from '../i18n/vi';
import { en } from '../i18n/en';
import { isAbsolutePath } from '../components/features';
import { validateOwnerPassword } from '../pages/Setup';

describe('formatDuration', () => {
  it('formats minutes and hours', () => {
    expect(formatDuration(0)).toBe('0:00');
    expect(formatDuration(5_000)).toBe('0:05');
    expect(formatDuration(125_000)).toBe('2:05');
    expect(formatDuration(3_723_000)).toBe('1:02:03');
  });
  it('rounds to the nearest second', () => {
    expect(formatDuration(59_600)).toBe('1:00');
  });
  it('handles null / invalid', () => {
    expect(formatDuration(null)).toBe('—');
    expect(formatDuration(undefined)).toBe('—');
    expect(formatDuration(-1)).toBe('—');
    expect(formatDuration(Number.NaN)).toBe('—');
  });
});

describe('formatBytes', () => {
  it('uses binary units', () => {
    expect(formatBytes(0)).toBe('0 B');
    expect(formatBytes(1023)).toBe('1023 B');
    expect(formatBytes(1024)).toBe('1 KB');
    expect(formatBytes(1536)).toBe('1.5 KB');
    expect(formatBytes(5 * 1024 ** 3)).toBe('5 GB');
    expect(formatBytes(250 * 1024 ** 2)).toBe('250 MB');
  });
  it('handles null / negative', () => {
    expect(formatBytes(null)).toBe('—');
    expect(formatBytes(-5)).toBe('—');
  });
});

describe('other formatters', () => {
  it('formatEta in both languages', () => {
    expect(formatEta(45)).toBe('45 giây');
    expect(formatEta(3720)).toBe('1 giờ 2 phút');
    expect(formatEta(125, 'en')).toBe('2m 5s');
    expect(formatEta(null)).toBe('—');
  });
  it('formatResolution', () => {
    expect(formatResolution(1920, 1080)).toBe('1920×1080');
    expect(formatResolution(null, 1080)).toBe('—');
  });
  it('secondsToMs', () => {
    expect(secondsToMs('')).toBeNull();
    expect(secondsToMs('1.5')).toBe(1500);
    expect(secondsToMs('2,25')).toBe(2250);
    expect(secondsToMs('abc')).toBeNaN();
    expect(secondsToMs('-1')).toBeNaN();
  });
  it('guessDeviceName', () => {
    expect(guessDeviceName('Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit Version/17.0 Mobile Safari/604.1')).toBe('Safari · iPhone');
    expect(guessDeviceName('Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/120.0 Safari/537.36 Edg/120.0')).toBe('Edge · Windows');
  });
});

describe('i18n translate', () => {
  const dicts = { vi: { a: 'Xin chào {name}', only_vi: 'Chỉ có tiếng Việt' }, en: { a: 'Hello {name}' } };
  it('translates and interpolates', () => {
    expect(translate('en', 'a', { name: 'Doto' }, dicts)).toBe('Hello Doto');
    expect(translate('vi', 'a', { name: 'Doto' }, dicts)).toBe('Xin chào Doto');
  });
  it('falls back to Vietnamese, then to the key', () => {
    expect(translate('en', 'only_vi', undefined, dicts)).toBe('Chỉ có tiếng Việt');
    expect(translate('en', 'missing.key', undefined, dicts)).toBe('missing.key');
  });
  it('leaves unknown placeholders untouched', () => {
    expect(translate('en', 'a', {}, dicts)).toBe('Hello {name}');
  });
  it('real dictionaries: English covers every Vietnamese key', () => {
    const missing = Object.keys(viDict).filter((k) => !(k in en));
    expect(missing).toEqual([]);
  });
  it('real dictionaries: owner & remote-host parity messages are present', () => {
    expect(translate('vi', 'host.only')).toBe('Chỉ dành cho Chủ sở hữu');
    expect(translate('vi', 'host.remoteBanner', { host: 'PC-HOST' })).toContain('PC-HOST');
  });
});

describe('misc helpers', () => {
  it('readDesktopToken', () => {
    expect(readDesktopToken('#desktop-token=XYZ')).toBe('XYZ');
    expect(readDesktopToken('#foo=1&desktop-token=abc')).toBe('abc');
    expect(readDesktopToken('')).toBeNull();
    expect(readDesktopToken('#desktop-token=')).toBeNull();
  });
  it('upsertById', () => {
    const l = [{ id: 'a', v: 1 }];
    expect(upsertById(l, { id: 'b', v: 2 })).toEqual([{ id: 'b', v: 2 }, { id: 'a', v: 1 }]);
    expect(upsertById(l, { id: 'a', v: 3 })).toEqual([{ id: 'a', v: 3 }]);
    expect(upsertById(undefined, { id: 'x', v: 0 }, false)).toEqual([{ id: 'x', v: 0 }]);
  });
  it('isAbsolutePath', () => {
    expect(isAbsolutePath('D:\\Videos')).toBe(true);
    expect(isAbsolutePath('/Users/a')).toBe(true);
    expect(isAbsolutePath('\\\\nas\\share')).toBe(true);
    expect(isAbsolutePath('videos')).toBe(false);
  });
  it('validateOwnerPassword', () => {
    expect(validateOwnerPassword('short1')).toBe('short');
    expect(validateOwnerPassword('abcdefghijk')).toBe('weak');
    expect(validateOwnerPassword('abcdefghij1')).toBeNull();
  });
});
