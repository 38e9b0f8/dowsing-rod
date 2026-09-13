export function formatUser(user) {
  const name = user.name;
  const active = user.isActive ? "yes" : "no";
  return `${name}: ${active}`;
}

export function renderUser(account) {
  const label = account.name;
  const enabled = account.isActive ? "yes" : "no";
  return `${label}: ${enabled}`;
}
