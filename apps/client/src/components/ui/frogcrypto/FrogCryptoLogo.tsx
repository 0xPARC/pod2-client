import { cn } from "@/lib/utils";

interface FrogCryptoLogoProps {
  className?: string;
}

export function FrogCryptoLogo({ className }: FrogCryptoLogoProps) {
  return (
    <div className={cn("flex items-center justify-center", className)}>
      <img 
        src="/src/assets/FROGCRYPTO.png" 
        alt="FROGCRYPTO" 
        className="h-[40px] w-auto"
      />
    </div>
  );
}
