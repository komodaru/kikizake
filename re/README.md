# Clean-room analysis of Windows' graphics and Win32 base DLLs

The owner allows reverse engineering Windows' graphics DLLs and the Win32 base DLLs (ntdll,
kernel32, kernelbase, user32) to make the models exact, done
clean-room:

* An analyst reads a DLL (the runner's own build, listed in `dlls.txt` by the `dlls` workflow
  and fetched from the Microsoft symbol server outside the repository) and writes only notes:
  the rule in words or pseudocode, its constants, the function it was found in. Notes stay in
  the project's shared folder (`kikizake-re/<api>/`), not in the repository. No notes,
  disassembly, decompiler output, copied code or binary is committed.
* The implementer writes the model and its tests from those notes and from Microsoft Learn,
  never from the binary.
* A rule found this way is still only `measured` once Windows' answers (golden/) confirm it;
  the note names the cases that do.
