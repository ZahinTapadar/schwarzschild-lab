"""Arithmetic check for pillars/research/facts.md. Pure stdlib."""
from math import pi, sqrt, sin, cos, asin, atan2, radians, degrees, log10

# constants (cgs)
k = 1.380649e-16; G = 6.674e-8; mH = 1.6735e-24; c = 2.998e10
pc = 3.0857e18; Msun = 1.989e33; Lsun = 3.828e33; yr = 3.156e7; Myr = 1e6 * yr
ly_per_pc = 3.2616
alphaB = 2.6e-13

print("== A. distance / geometry ==")
for d in (1.706, 1.74, 2.0):
    print(f"d={d} kpc = {d*1e3*ly_per_pc:.0f} ly")
# Galactic coordinates of RA 18h18m48s, Dec -13d49m (J2000) via IAU rotation
ra = radians((18 + 18/60 + 48/3600) * 15); dec = radians(-(13 + 49/60))
ra_gp, dec_gp, l_ncp = radians(192.85948), radians(27.12825), radians(122.93192)
b = asin(sin(dec)*sin(dec_gp) + cos(dec)*cos(dec_gp)*cos(ra - ra_gp))
l = l_ncp - atan2(cos(dec)*sin(ra - ra_gp),
                  sin(dec)*cos(dec_gp) - cos(dec)*sin(dec_gp)*cos(ra - ra_gp))
print(f"l={degrees(l) % 360:.2f} deg, b={degrees(b):.2f} deg")
# angular scale
for d in (1.74, 2.0):
    print(f"1 arcmin at {d} kpc = {d*1e3*radians(1/60):.3f} pc")
# NASA 'about 5 ly' tallest pillar (at their 2 kpc) -> rescale to 1.74 kpc
L_nasa_pc = 5 / ly_per_pc
print(f"5 ly = {L_nasa_pc:.2f} pc; angular = {degrees(L_nasa_pc/2000)*60:.2f} arcmin; at 1.74 kpc -> {L_nasa_pc*1.74/2:.2f} pc = {L_nasa_pc*1.74/2*ly_per_pc:.1f} ly")
# nebula 70'x50' and 30' at 1.74 kpc
for a in (30, 50, 70):
    print(f"{a} arcmin at 1.74 kpc = {1740*radians(a/60):.1f} pc")

print("\n== B. Q(H) from Martins+2005 Table 4 (class V) for Stoop+2023 types ==")
logQ = {"O3":49.64,"O4":49.44,"O5":49.22,"O5.5":49.10,"O6":48.99,"O6.5":48.88,"O7":48.75,
        "O7.5":48.61,"O8":48.44,"O8.5":48.27,"O9":48.06,"O9.5":47.88}
logQ["O3.5"] = (logQ["O3"] + logQ["O4"]) / 2
stars = {"W205/HD168076": ["O3.5","O7.5"], "W175": ["O4","O7.5"], "W197": ["O6.5"],
         "W161": ["O8.5"], "W166": ["O9"], "W222": ["O7"], "W246 (O7 II, use V)": ["O7"],
         "W401": ["O7","O8"], "W314": ["O7"]}
tot = 0
for s, comps in stars.items():
    q = sum(10**logQ[t] for t in comps); tot += q
    print(f"  {s:22s} {comps} Q={q:.2e}")
print(f"  sum Q = {tot:.2e} s^-1 (log {log10(tot):.2f}); HD168076 share {(10**logQ['O3.5']+10**logQ['O7.5'])/tot:.0%}")
Q = 7e49
print(f"adopted Q = {Q:.1e}, log = {log10(Q):.2f}")

print("\n== F. Stromgren radius ==")
for n in (10, 100, 1000):
    Rs = (3*Q/(4*pi*n**2*alphaB))**(1/3)
    print(f"n={n:5d}: Rs={Rs:.2e} cm = {Rs/pc:.2f} pc ; t_rec=1/(n aB)={1/(n*alphaB)/yr:.0f} yr")
for Qx in (6.25e49, 8.43e49):
    print(f"  (Q={Qx:.2e}, n=100: Rs={(3*Qx/(4*pi*1e4*alphaB))**(1/3)/pc:.2f} pc)")
# fraction of photons used up inside 2 pc for n=100
print(f"  fraction of Q absorbed inside 2 pc at n=100: {(4/3*pi*(2*pc)**3*100**2*alphaB)/Q:.1%}")

print("\n== F. flux at pillar tips ==")
F = {}
for dpc in (2.0, 3.0):
    F[dpc] = Q/(4*pi*(dpc*pc)**2)
    print(f"d={dpc} pc: F={F[dpc]:.2e} cm^-2 s^-1")

print("\n== F. ionization-front density n_IF = sqrt(F/(aB r)) ==")
nIF = {}
for dpc in (2.0, 3.0):
    for rc in (0.05, 0.1, 0.2):
        nIF[(dpc, rc)] = sqrt(F[dpc]/(alphaB*rc*pc))
        print(f"d={dpc} pc, r_c={rc} pc: n_IF={nIF[(dpc, rc)]:.0f} cm^-3")

print("\n== F. photoevaporation mass flux ==")
mu_i = 1.4  # mass per H nucleus incl. He
cs_flow = 1e6
for n in (1000, 1400, 2000):
    flux = mu_i*mH*n*cs_flow  # g cm^-2 s^-1
    per = flux*Myr*pc**2/Msun
    print(f"n_IF={n}: {flux:.2e} g cm^-2 s^-1 = {per:.0f} Msun Myr^-1 pc^-2")
n = 1400; per = mu_i*mH*n*cs_flow*Myr*pc**2/Msun
for r in (0.1, 0.15):
    A = pi*r**2
    print(f"  head radius {r} pc, area {A:.3f} pc^2 -> {per*A:.0f} Msun/Myr per head; x4 heads {4*per*A:.0f}")
# McLeod 2015 check: Mdot = M v / l with M from [SII]? they give 70; lifetime 200/70
print(f"McLeod lifetime 200/70 = {200/70:.1f} Myr; Karim masses sum = {103+78+103+18} Msun -> /70 = {(103+78+103+18)/70:.1f} Myr")

print("\n== F. sound speeds (isothermal c = sqrt(kT/(mu mH))) ==")
for T, mu in ((10, 2.33), (20, 2.33), (100, 1.27), (1e4, 0.61)):
    print(f"T={T:g} K, mu={mu}: c={sqrt(k*T/(mu*mH))/1e5:.2f} km/s (adiabatic x1.29 for gamma=5/3: {sqrt(5/3*k*T/(mu*mH))/1e5:.2f})")

print("\n== F. pressures ==")
ne, Te = 1400, 1e4
P_ion = 2*ne*k*Te
print(f"ionized skin 2 n_e k T: P/k = {P_ion/k:.2e} K cm^-3")
for n, T in ((1e4, 20), (1.3e5, 20)):
    print(f"molecular thermal n={n:.1e}, T={T}: P/k={n*T:.1e}")
Lcl = 0
logL = {"O3.5":(5.84+5.67)/2,"O4":5.67,"O6.5":5.23,"O7":5.14,"O7.5":5.05,"O8":4.96,"O8.5":4.86,"O9":4.77}
for comps in stars.values():
    Lcl += sum(10**logL[t] for t in comps)
print(f"cluster L_bol (O stars, Martins V) = {Lcl:.2e} Lsun")
for dpc in (2.0, 3.0):
    Prad = Lcl*Lsun/(4*pi*(dpc*pc)**2*c)
    print(f"  d={dpc} pc: P_rad/k = {Prad/k:.2e} K cm^-3 ; P_ion/P_rad = {P_ion/Prad:.0f}")
# ram pressure of ionized flow rho v^2 at v=c_s ~ equals thermal (by construction)

print("\n== E. Jeans / free-fall (mu=2.33, rho = mu mH n) ==")
for T in (10, 20):
    for n in (1e4, 1e5, 1.3e5):
        rho = 2.33*mH*n
        cs = sqrt(k*T/(2.33*mH))
        lamJ = sqrt(pi*cs**2/(G*rho))
        MJ = 4/3*pi*rho*(lamJ/2)**3          # sphere of diameter lambda_J
        MJ2 = (5*k*T/(G*2.33*mH))**1.5*(3/(4*pi*rho))**0.5  # radius-form Jeans mass
        tff = sqrt(3*pi/(32*G*rho))
        print(f"T={T} n={n:.1e}: lambda_J={lamJ/pc:.3f} pc ({lamJ/1.496e13:.0f} AU), "
              f"M_J(sphere d=lamJ)={MJ/Msun:.2f}, M_J(5kT/G form)={MJ2/Msun:.2f} Msun, t_ff={tff/yr/1e5:.2f}e5 yr")

print("\n== G. CCM89 extinction A_lambda/A_V, R_V=3.1 ==")
def ccm(lam_um, Rv=3.1):
    x = 1/lam_um
    if 0.3 <= x <= 1.1:
        a = 0.574*x**1.61; b = -0.527*x**1.61
    elif 1.1 < x <= 3.3:
        y = x - 1.82
        a = 1 + 0.17699*y - 0.50447*y**2 - 0.02427*y**3 + 0.72085*y**4 + 0.01979*y**5 - 0.77530*y**6 + 0.32999*y**7
        b = 1.41338*y + 2.28305*y**2 + 1.07233*y**3 - 5.38434*y**4 - 0.62251*y**5 + 5.30260*y**6 - 2.09002*y**7
    elif x < 0.3:  # outside CCM validity; power-law extrapolation
        a = 0.574*x**1.61; b = -0.527*x**1.61
    return a + b/Rv
for lam in (0.5007, 0.55, 0.6563, 0.6724, 1.25, 2.2, 4.4):
    print(f"  {lam:.4f} um: A/A_V = {ccm(lam):.3f}")
print(f"  R_V=3.75 at 0.6563: {ccm(0.6563, 3.75):.3f}")
print(f"  Indebetouw05: A_4.5/A_V = 0.43*A_K/A_V = {0.43*ccm(2.2):.3f}")
for Av in (2.6, 3.2, 20, 50):
    print(f"  A_V={Av}: A_Ha={ccm(0.6563)*Av:.1f}, A_K={ccm(2.2)*Av:.1f}, A_4.4(CCM extrap)={ccm(4.4)*Av:.1f}, A_4.5(Indebetouw)={0.43*ccm(2.2)*Av:.1f} mag")
# column -> A_V: N_H = 2.2e21 A_V (Guver & Ozel 2009)
for NH in (67e21, 114e21, 286e21):
    print(f"  N_H={NH:.2e} -> A_V ~ {NH/2.21e21:.0f} mag")

print("\n== G. surface brightness ==")
R_cgs = 1e6/(4*pi) * (radians(1/3600))**2 * (6.626e-27*c/656.3e-7)
print(f"1 Rayleigh at Ha = {R_cgs:.2e} erg s^-1 cm^-2 arcsec^-2")
for ne_, L in ((100, 5), (2000, 0.02), (1400, 0.05)):
    EM = ne_**2*L
    print(f"  n_e={ne_}, L={L} pc: EM={EM:.1e} pc cm^-6 -> I(Ha)~{EM/2.75:.1e} R = {EM/2.75*R_cgs:.1e} erg/s/cm2/arcsec2")
# SITELLE Table 2 integrated Ha flux 8.2e-10 in a 3'11" aperture
for label, rad in (("diameter", 191/2), ("radius", 191)):
    sb = 8.2e-10/(pi*rad**2)
    print(f"  SITELLE 3'11\" as {label}: observed SB = {sb:.1e} erg/s/cm2/arcsec2 = {sb/R_cgs:.0f} R; dereddened A_Ha=2.1 -> {sb/R_cgs*10**(0.4*2.1):.0f} R")
# map to R-band mag/arcsec^2 (line flux spread over Johnson R; F0 ~ 2.18e-9 erg/s/cm2/A * 1580 A)
F0R = 2.18e-9*1580
for sb in (2.9e-14, 7.1e-15, 1e-13):
    mu_ = -2.5*log10(sb/F0R)
    print(f"  SB={sb:.1e} -> ~{mu_:.1f} R-mag/arcsec^2 ; luminance if it were V: {10.8e4*10**(-0.4*mu_):.1e} cd/m^2")

print("\n== F. stellar lifetimes (Ekstrom+2012, total H+He+C burning) ==")
for M, tnr, tr in ((85, 3.37, 4.03), (60, 3.93, 4.82), (40, 4.92, 6.12), (20, 8.62, 10.35)):
    print(f"  {M} Msun: {tnr}-{tr} Myr -> minus cluster age 1.3-2 Myr = {tnr-2:.1f}-{tr-1.3:.1f} Myr from now")

print("\n== A. light travel ==")
print(f"1.74 kpc = {1740*ly_per_pc:.0f} ly -> light left ~{1740*ly_per_pc:.0f} yr ago")
print(f"HST jet: 60e9 miles/19 yr = {60e9*1.609/(19*yr):.0f} km/s; 450000 mph = {450000*1.609/3600:.0f} km/s")
