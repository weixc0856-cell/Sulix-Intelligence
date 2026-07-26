-- Seed initial filter_rules for Sulix Feed scoring.
-- Each rule_json is a serialized rules::Rule struct (serde tagged JSON).
-- Audience "default" is loaded by the queue consumer.

INSERT OR IGNORE INTO filter_rules (name, rule_json, audience_tag, enabled) VALUES
(
  'Frontier AI models',
  '{"name":"Frontier AI models","audience_tag":"default","condition":{"type":"any","conditions":[{"type":"keyword_includes","field":"title","keyword":"GPT"},{"type":"keyword_includes","field":"title","keyword":"Claude"},{"type":"keyword_includes","field":"title","keyword":"DeepSeek"},{"type":"keyword_includes","field":"title","keyword":"Gemini"},{"type":"keyword_includes","field":"title","keyword":"Llama"}]},"score_delta":3.0}',
  'default',
  1
),
(
  'AI safety and alignment',
  '{"name":"AI safety and alignment","audience_tag":"default","condition":{"type":"any","conditions":[{"type":"keyword_includes","field":"title","keyword":"safety"},{"type":"keyword_includes","field":"title","keyword":"alignment"},{"type":"keyword_includes","field":"title","keyword":"AGI"},{"type":"keyword_includes","field":"title","keyword":"capability"},{"type":"keyword_includes","field":"summary","keyword":"safety"}]},"score_delta":2.0}',
  'default',
  1
),
(
  'Security vulnerabilities',
  '{"name":"Security vulnerabilities","audience_tag":"default","condition":{"type":"any","conditions":[{"type":"keyword_includes","field":"title","keyword":"vulnerability"},{"type":"keyword_includes","field":"title","keyword":"CVE"},{"type":"keyword_includes","field":"title","keyword":"zero-day"},{"type":"keyword_includes","field":"title","keyword":"exploit"},{"type":"keyword_includes","field":"title","keyword":"breach"},{"type":"keyword_includes","field":"title","keyword":"ransomware"}]},"score_delta":2.5}',
  'default',
  1
),
(
  'AI regulation and policy',
  '{"name":"AI regulation and policy","audience_tag":"default","condition":{"type":"any","conditions":[{"type":"keyword_includes","field":"title","keyword":"regulation"},{"type":"keyword_includes","field":"title","keyword":"policy"},{"type":"keyword_includes","field":"title","keyword":"AI Act"},{"type":"keyword_includes","field":"title","keyword":"governance"},{"type":"keyword_includes","field":"title","keyword":"legislation"},{"type":"keyword_includes","field":"title","keyword":"executive order"}]},"score_delta":2.0}',
  'default',
  1
),
(
  'Product and release announcements',
  '{"name":"Product and release announcements","audience_tag":"default","condition":{"type":"any","conditions":[{"type":"keyword_includes","field":"title","keyword":"announce"},{"type":"keyword_includes","field":"title","keyword":"release"},{"type":"keyword_includes","field":"title","keyword":"launch"},{"type":"keyword_includes","field":"title","keyword":"introducing"},{"type":"keyword_includes","field":"title","keyword":"new model"},{"type":"keyword_includes","field":"title","keyword":"beta"}]},"score_delta":1.5}',
  'default',
  1
),
(
  'Research and benchmarks',
  '{"name":"Research and benchmarks","audience_tag":"default","condition":{"type":"any","conditions":[{"type":"keyword_includes","field":"title","keyword":"research"},{"type":"keyword_includes","field":"title","keyword":"benchmark"},{"type":"keyword_includes","field":"title","keyword":"SOTA"},{"type":"keyword_includes","field":"title","keyword":"paper"},{"type":"keyword_includes","field":"title","keyword":"technical report"}]},"score_delta":1.5}',
  'default',
  1
);
