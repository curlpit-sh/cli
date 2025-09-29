export interface PlaygroundExample {
  label: string;
  value: string;
  curl: string;
  variables: string;
}

export const playgroundExamples: PlaygroundExample[] = [
  {
    label: "HTTPBin with Variables",
    value: "httpbin",
    curl: `# GET request with templated variables (like curlpit)
GET {API_BASE}/get
User-Agent: {USER_AGENT}
Accept: application/json
Authorization: Bearer {API_TOKEN}`,
    variables: `# Environment variables (these get interpolated)
API_BASE=https://httpbin.org
USER_AGENT=curlpit-playground/1.0
API_TOKEN=demo-token-12345`,
  },
  {
    label: "GitHub API",
    value: "github",
    curl: `# GitHub API with variable expansion
GET {API_BASE}/repos/{OWNER}/{REPO}
Accept: application/vnd.github.v3+json
User-Agent: {USER_AGENT}
Authorization: Bearer {GITHUB_TOKEN}`,
    variables: `# GitHub API configuration
API_BASE=https://api.github.com
OWNER=curlpit-sh
REPO=cli
USER_AGENT=curlpit/1.0
GITHUB_TOKEN=ghp_your_token_here`,
  },
  {
    label: "POST with Body",
    value: "post",
    curl: `# POST with JSON body and variables
POST {API_BASE}/posts
Content-Type: application/json
Accept: application/json
X-Request-ID: {REQUEST_ID}

{
  "title": "{POST_TITLE}",
  "body": "Posted via curlpit at {TIMESTAMP}",
  "userId": {USER_ID}
}`,
    variables: `# JSONPlaceholder POST example
API_BASE=https://jsonplaceholder.typicode.com
POST_TITLE=Test Post from Curlpit
USER_ID=1
REQUEST_ID=req_${Date.now()}
TIMESTAMP=${new Date().toISOString()}`,
  },
  {
    label: "JSONPlaceholder",
    value: "placeholder",
    curl: `# JSONPlaceholder API (CORS-friendly)
GET {BASE_URL}/users/{USER_ID}
Accept: application/json
User-Agent: {APP_NAME}/{APP_VERSION}
X-Custom-Header: {CUSTOM_VALUE}`,
    variables: `# Works in browser (CORS-enabled API)
BASE_URL=https://jsonplaceholder.typicode.com
USER_ID=1
APP_NAME=curlpit-playground
APP_VERSION=1.0.0
CUSTOM_VALUE=demo-${Math.random().toString(36).substring(7)}`,
  },
];
