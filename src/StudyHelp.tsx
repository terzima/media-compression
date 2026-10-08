export default function StudyHelp({audio=false}:{audio?:boolean}) {
 return <details className="study-help">
  <summary>How to choose a result</summary>
  <p>A study encodes real copies from your original and measures their sizes. It tests the displayed settings; it does not automatically find an invisible quality limit.</p>
  {audio?<>
   <p><strong>Same samples:</strong> choose FLAC for supported 16-/24-bit integer sources. Lossless compression cannot recover information already lost in an MP3 or AAC source.</p>
   <p><strong>Smaller audio:</strong> run Quick study, switch Original/Candidate at the same position, and listen to important passages. Timing diagnostics do not measure audibility.</p>
  </>:<>
   <p><strong>Exactly the same pixels:</strong> choose PNG or WebP, then Compression type → Lossless. Run Quick study. Inspect a result and check Decoded pixels → Identical. Try the other supported lossless format to compare sizes.</p>
   <p><strong>Visually similar, smaller:</strong> choose WebP, then Advanced → Lossy and study quality settings such as 98,95,90. Run Quick study, inspect SSIM and alpha errors, and compare at 100% on light/dark backgrounds.</p>
   <p>SSIM is a structural similarity measurement, not a percent quality or proof that you cannot see a change. Encoder quality 95 does not mean 95% similarity.</p>
  </>}
  <p>Click a candidate to inspect it; its checkbox selects it for Export selected. Review the checkboxes before exporting. Originals stay unchanged.</p>
  <p><strong>Ask your connected agent:</strong> “Help me make this file smaller. Ask only what you need, then run it and give me the result.” Provide a local file or folder and where to save copies. The agent uses your existing AI client; this app supplies local tools.</p>
 </details>;
}
