// A small test runner (node:test needs Node 18).
const tests = [];

export function test(name, fn) {
  tests.push({ name, fn });
}

export async function run() {
  let failed = 0;
  for (const { name, fn } of tests) {
    try {
      await fn();
      console.log(`ok   ${name}`);
    } catch (err) {
      failed++;
      console.log(`FAIL ${name}\n     ${err.stack || err}`);
    }
  }
  console.log(`\n${tests.length - failed} passed, ${failed} failed`);
  if (failed) process.exitCode = 1;
}
