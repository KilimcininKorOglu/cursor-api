const dsl_content: &str = r#"
meta indent = "  "
meta fallback = "unknown"

g
  p => openai
  e => google
  r => xai

o
  1
  3
  4 => openai

c
  l => anthropic
  u
  o => cursor

d
  e
    e => deepseek

a
  @prove > 26 [(9, f)] => fireworks
"#;
