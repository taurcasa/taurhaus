const [majorRaw = "0", minorRaw = "0"] = process.versions.node.split(".");
const major = Number.parseInt(majorRaw, 10);
const minor = Number.parseInt(minorRaw, 10);

const supported =
  (major === 20 && minor >= 12) ||
  major === 22 ||
  major >= 24;

if (!supported) {
  console.error(
    `Unsupported Node.js runtime ${process.versions.node}. ` +
      "taurhaus frontend lint tooling requires Node 20.12+, 22.x, or 24+."
  );
  process.exit(1);
}
