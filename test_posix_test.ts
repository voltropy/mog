import { compile } from './src/compiler.ts';

const source = `fn main() -> i64 {
  fd: i64 = open("test.txt", O_CREAT | O_WRONLY, 0644);
  if (fd == -1) {
    return 1;
  }
  write(fd, "Hello POSIX!\\n", 12);
  close(fd);
  return 0;
}`;

compile(source).then(result => {
  if (result.errors.length > 0) {
    console.error("Errors:");
    result.errors.forEach(e => console.error(`  Line ${e.line}: ${e.message}`));
    process.exit(1);
  } else {
    console.log("=== Checking string constants section ===");
    const lines = result.llvmIR.split('\n');
    let strLine = -1, funcLine = -1;
    lines.forEach((line, i) => {
      if (line.includes('; String constants')) strLine = i;
      if (line.includes('define i64 @program_user')) funcLine = i;
    });
    console.log(`String constants section: line ${strLine}`);
    console.log(`Function declaration: line ${funcLine}`);
    const first = strLine < funcLine && strLine >= 0 && funcLine >= 0;
    console.log(`String constants come first: ${first ? 'YES ✓' : 'NO ✗'}`);

    console.log("\nString constants:");
    lines.forEach((line, i) => {
      if (i >= strLine && i < strLine + 10 && line.startsWith('@str')) {
        console.log(`  ${i}: ${line}`);
      }
    });
  }
});
