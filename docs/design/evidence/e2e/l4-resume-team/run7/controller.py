"""Run 7 uses the corrected run6 controller; prior runtime evidence is untouched."""
from pathlib import Path
import sys
sys.path.insert(0,str(Path(__file__).resolve().parent.parent/'run6'))
import controller
controller.OUT=Path(__file__).resolve().parent
if __name__=='__main__': raise SystemExit(controller.main())
