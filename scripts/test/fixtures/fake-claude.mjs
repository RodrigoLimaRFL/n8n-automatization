// Stand-in for the claude CLI: reads the prompt from stdin and prints claude's JSON.
let prompt = '';
process.stdin.on('data', (d) => (prompt += d));
process.stdin.on('end', () => {
  if (prompt.includes('LIMIT')) {
    console.log(JSON.stringify({ is_error: true, result: "You've hit your session limit" }));
  } else {
    console.log(JSON.stringify({
      result: `echo:${prompt}`,
      usage: { input_tokens: 10, cache_creation_input_tokens: 5, cache_read_input_tokens: 1, output_tokens: 7 },
      duration_ms: 12,
    }));
  }
});
