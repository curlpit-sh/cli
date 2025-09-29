export type EnvVars = Record<string, string>;

const ENV_LINE = /^([A-Z_][A-Z0-9_\-.]*)\s*=\s*(.*)$/;

export function parseEnvVariables(content: string): EnvVars {
  const vars: EnvVars = {};
  const lines = content.split("\n");

  for (const line of lines) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) {
      continue;
    }

    const match = trimmed.match(ENV_LINE);
    if (!match) {
      continue;
    }

    const [, key, rawValue] = match;
    vars[key] = evaluateBasicTemplate(rawValue);
  }

  return vars;
}

function evaluateBasicTemplate(value: string): string {
  if (!value.includes("${")) {
    return value;
  }

  return value.replace(/\$\{([^}]+)\}/g, (_, expr: string) => {
    if (expr === "Date.now()") {
      return String(Date.now());
    }
    if (expr === "Math.random()") {
      return Math.random().toString();
    }
    if (expr.startsWith("new Date().toISOString")) {
      return new Date().toISOString();
    }
    return expr;
  });
}

export function formatImportedVariables(
  templateVars: EnvVars,
  envVars: EnvVars,
): string {
  const lines: string[] = ["# Template variables discovered from import"];

  const templateEntries = Object.entries(templateVars).sort((a, b) =>
    a[0].localeCompare(b[0]),
  );
  const envEntries = Object.entries(envVars).sort((a, b) => a[0].localeCompare(b[0]));

  templateEntries.forEach(([key, value]) => {
    lines.push(`${key}=${value}`);
  });

  const envOnly = envEntries.filter(([key, value]) => templateVars[key] !== value);

  if (envOnly.length > 0) {
    lines.push("", "# Environment variables promoted from secrets");
    envOnly.forEach(([key, value]) => {
      lines.push(`${key}=${value}`);
    });
  }

  return lines.join("\n");
}
