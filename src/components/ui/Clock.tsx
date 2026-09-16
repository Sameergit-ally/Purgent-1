import { useEffect, useState } from "react";

export function Clock() {
  const [now, setNow] = useState<Date>(() => new Date());

  useEffect(() => {
    const t = setInterval(() => setNow(new Date()), 1000);
    return () => clearInterval(t);
  }, []);

  return <span>UTC {now.toISOString().slice(11, 19)}</span>;
}