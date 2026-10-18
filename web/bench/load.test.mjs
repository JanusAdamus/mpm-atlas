// Response framing of the load generator: a miscounted body would desynchronize keep-alive.
import { test } from "node:test";
import assert from "node:assert/strict";
import { responseLength } from "./load.mjs";

const b = (s) => Buffer.from(s, "latin1");

test("content-length responses end exactly after the body", () => {
  const r = "HTTP/1.1 200 OK\r\ncontent-length: 5\r\n\r\nhello";
  assert.equal(responseLength(b(r)), r.length);
  assert.equal(responseLength(b(r.slice(0, -1))), -1);
  assert.equal(responseLength(b(r + "HTTP/1.1 200")), r.length);
});

test("chunked responses end after the zero chunk", () => {
  const r = "HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n3\r\nabc\r\na\r\n0123456789\r\n0\r\n\r\n";
  assert.equal(responseLength(b(r)), r.length);
  assert.equal(responseLength(b(r.slice(0, -2))), -1);
});

test("bodiless responses (304) end after the headers", () => {
  const r = "HTTP/1.1 304 Not Modified\r\netag: W/\"x\"\r\n\r\n";
  assert.equal(responseLength(b(r)), r.length);
});
