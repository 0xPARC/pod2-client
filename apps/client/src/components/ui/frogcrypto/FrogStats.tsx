import { cn } from "@/lib/utils";

interface FrogStatsProps {
  jump?: number;
  vibe?: string;
  speed?: number;
  intelligence?: number;
  beauty?: number;
  className?: string;
}

export function FrogStats({ 
  jump, 
  vibe, 
  speed, 
  intelligence, 
  beauty, 
  className 
}: FrogStatsProps) {
  return (
    <div className={cn("space-y-3 px-8", className)}>
      {/* First row: JUMP, VIBE */}
      <div className="grid grid-cols-2 px-8">
        <div className="text-center">
          <div 
            style={{
              color: '#293231',
              textAlign: 'center',
              fontFamily: 'var(--font-sans)',
              fontSize: '14px',
              fontStyle: 'normal',
              fontWeight: 600,
              lineHeight: 'normal',
              letterSpacing: '0.28px',
              textTransform: 'uppercase'
            }}
          >
            JUMP
          </div>
          <div 
            style={{
              color: '#293231',
              textAlign: 'center',
              fontFamily: 'var(--font-sans)',
              fontSize: '14px',
              fontStyle: 'normal',
              fontWeight: 400,
              lineHeight: 'normal',
              letterSpacing: '0.28px'
            }}
          >
            {jump ?? "???"}
          </div>
        </div>
        
        <div className="text-center">
          <div 
            style={{
              color: '#293231',
              textAlign: 'center',
              fontFamily: 'var(--font-sans)',
              fontSize: '14px',
              fontStyle: 'normal',
              fontWeight: 600,
              lineHeight: 'normal',
              letterSpacing: '0.28px',
              textTransform: 'uppercase'
            }}
          >
            VIBE
          </div>
          <div 
            style={{
              color: '#293231',
              textAlign: 'center',
              fontFamily: 'var(--font-sans)',
              fontSize: '14px',
              fontStyle: 'normal',
              fontWeight: 400,
              lineHeight: 'normal',
              letterSpacing: '0.28px'
            }}
          >
            {vibe ?? "???"}
          </div>
        </div>
      </div>
      
      {/* Second row: SPED, INTL, BUTY */}
      <div className="grid grid-cols-3 px-6">
        <div className="text-center">
          <div 
            style={{
              color: '#293231',
              textAlign: 'center',
              fontFamily: 'var(--font-sans)',
              fontSize: '14px',
              fontStyle: 'normal',
              fontWeight: 600,
              lineHeight: 'normal',
              letterSpacing: '0.28px',
              textTransform: 'uppercase'
            }}
          >
            SPED
          </div>
          <div 
            style={{
              color: '#293231',
              textAlign: 'center',
              fontFamily: 'var(--font-sans)',
              fontSize: '14px',
              fontStyle: 'normal',
              fontWeight: 400,
              lineHeight: 'normal',
              letterSpacing: '0.28px'
            }}
          >
            {speed ?? "???"}
          </div>
        </div>
        
        <div className="text-center">
          <div 
            style={{
              color: '#293231',
              textAlign: 'center',
              fontFamily: 'var(--font-sans)',
              fontSize: '14px',
              fontStyle: 'normal',
              fontWeight: 600,
              lineHeight: 'normal',
              letterSpacing: '0.28px',
              textTransform: 'uppercase'
            }}
          >
            INTL
          </div>
          <div 
            style={{
              color: '#293231',
              textAlign: 'center',
              fontFamily: 'var(--font-sans)',
              fontSize: '14px',
              fontStyle: 'normal',
              fontWeight: 400,
              lineHeight: 'normal',
              letterSpacing: '0.28px'
            }}
          >
            {intelligence ?? "???"}
          </div>
        </div>
        
        <div className="text-center">
          <div 
            style={{
              color: '#293231',
              textAlign: 'center',
              fontFamily: 'var(--font-sans)',
              fontSize: '14px',
              fontStyle: 'normal',
              fontWeight: 600,
              lineHeight: 'normal',
              letterSpacing: '0.28px',
              textTransform: 'uppercase'
            }}
          >
            BUTY
          </div>
          <div 
            style={{
              color: '#293231',
              textAlign: 'center',
              fontFamily: 'var(--font-sans)',
              fontSize: '14px',
              fontStyle: 'normal',
              fontWeight: 400,
              lineHeight: 'normal',
              letterSpacing: '0.28px'
            }}
          >
            {beauty ?? "???"}
          </div>
        </div>
      </div>
    </div>
  );
}
