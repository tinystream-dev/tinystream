// SPDX-License-Identifier: AGPL-3.0-or-later

import { expect, test } from 'bun:test'
import { origins, resolve } from './address'

test('a bare host tries https, then http', () => {
  expect(origins('tiny.lan')).toEqual(['https://tiny.lan', 'http://tiny.lan'])
  expect(origins(' 10.0.0.2:3000 ')).toEqual(['https://10.0.0.2:3000', 'http://10.0.0.2:3000'])
  expect(origins('[::1]:3000')).toEqual(['https://[::1]:3000', 'http://[::1]:3000'])
})

test('a full URL keeps its scheme and loses its path', () => {
  expect(origins('HTTPS://TV.Example.com/')).toEqual(['https://tv.example.com'])
  expect(origins('http://tiny.lan:3000/settings?x=1')).toEqual(['http://tiny.lan:3000'])
})

test('nonsense is no address', () => {
  expect(origins('')).toEqual([])
  expect(origins('ftp://tiny.lan')).toEqual([])
  expect(origins('two words')).toEqual([])
  expect(origins(':3000')).toEqual([])
})

test('server paths resolve against the server', () => {
  expect(resolve('https://tv.example.com', '/api/images/1')).toBe('https://tv.example.com/api/images/1')
  expect(resolve('https://tv.example.com', 'https://cdn.example.com/a.jpg')).toBe('https://cdn.example.com/a.jpg')
})
